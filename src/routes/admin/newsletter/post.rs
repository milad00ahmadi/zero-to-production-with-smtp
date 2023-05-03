use std::fmt::Formatter;

use actix_web::body::BoxBody;
use actix_web::http::header::HeaderMap;
use actix_web::http::StatusCode;
use actix_web::web::ReqData;
use actix_web::{web, HttpRequest, HttpResponse, ResponseError};
use actix_web_flash_messages::FlashMessage;
use anyhow::Context;
use lettre::AsyncTransport;
use linkify::Span;
use secrecy::Secret;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::authentication::{validate_credentials, AuthError, Credentials, UserId};
use crate::domain::SubscriberEmail;
use crate::email_client::EmailClient;
use crate::idempotency::{get_saved_response, save_response, try_processing, IdempotencyKey, NextAction};
use crate::routes::admin::dashboard::get_username;
use crate::routes::{error_chain_fmt, newsletter};
use crate::utils::{e400, e500, see_other};

#[derive(serde::Deserialize)]
pub struct FormData {
    title: String,
    html_content: String,
    text_content: String,
    idempotency_key: String,
}

#[tracing::instrument(
    name = "Publish a newsletter issue",
    skip_all,
    fields(user_id=%*user_id)
)]
pub async fn publish_newsletter<T>(
    form: web::Form<FormData>,
    pool: web::Data<PgPool>,
    user_id: ReqData<UserId>,
) -> Result<HttpResponse, actix_web::Error> {
    let user_id = user_id.into_inner();
    let FormData {
        title,
        text_content,
        html_content,
        idempotency_key,
    } = form.0;
    let idempotency_key: IdempotencyKey = idempotency_key.try_into().map_err(e400)?;
    let mut transaction = match try_processing(&pool, &idempotency_key, *user_id).await.map_err(e500)? {
        NextAction::StartProcessing(t) => t,
        NextAction::ReturnSavedResponse(saved_response) => {
            success_message().send();
            return Ok(saved_response);
        },
    };
    let issue_id = insert_newsletter_issue(&mut transaction, &title, &text_content, &html_content)
        .await
        .context("Failed to store newsletter issue details")
        .map_err(e500)?;
    enqueue_delivery_tasks(&mut transaction, issue_id)
        .await
        .context("Faiedl to enqueue delivery tasks")
        .map_err(e500)?;

    let response = see_other("/admin/newsletters");
    let response = save_response(transaction, &idempotency_key, *user_id, response)
        .await
        .map_err(e500)?;
    success_message().send();
    Ok(response)
}

fn success_message() -> FlashMessage {
    FlashMessage::info(
        "The newsletter issue has been accepted - \
        emails will go out shortly."
    )
}

impl std::fmt::Display for SubscriberEmail {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[tracing::instrument(skip_all)]
async fn insert_newsletter_issue(
    transaction: &mut Transaction<'_, Postgres>,
    title: &str,
    text_content: &str,
    html_content: &str,
) -> Result<Uuid, sqlx::Error> {
    let newsletter_issue_id = Uuid::new_v4();
    sqlx::query!(
        r#"
        INSERT INTO newsletter_issues (
            newsletter_issue_id,
            title,
            text_content,
            html_content,
            published_at
        )
        VALUES ($1, $2, $3, $4, now())
        "#,
        newsletter_issue_id,
        title,
        text_content,
        html_content,
    )
    .execute(transaction)
    .await?;
    Ok(newsletter_issue_id)
}

#[tracing::instrument(skip_all)]
async fn enqueue_delivery_tasks(
    transaction: &mut Transaction<'_, Postgres>,
    newsletter_issue_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        INSERT INTO issue_delivery_queue (
            newsletter_issue_id,
            subscriber_email
        )
        SELECT $1, email
        FROM subscriptions
        WHERE status = 'confirmed'
        "#,
        newsletter_issue_id
    )
    .execute(transaction)
    .await?;
    Ok(())
}
