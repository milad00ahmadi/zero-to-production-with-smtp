use std::fmt::Formatter;

use actix_web::body::BoxBody;
use actix_web::http::header::HeaderMap;
use actix_web::http::StatusCode;
use actix_web::web::ReqData;
use actix_web::{web, HttpRequest, HttpResponse, ResponseError};
use actix_web_flash_messages::FlashMessage;
use anyhow::Context;
use lettre::AsyncTransport;
use reqwest::header;
use reqwest::header::HeaderValue;
use secrecy::Secret;
use sqlx::PgPool;

use crate::authentication::{validate_credentials, AuthError, Credentials, UserId};
use crate::domain::SubscriberEmail;
use crate::email_client::EmailClient;
use crate::routes::admin::dashboard::get_username;
use crate::routes::error_chain_fmt;
use crate::utils::{e500, see_other};

#[derive(serde::Deserialize)]
pub struct FormData {
    title: String,
    html_content: String,
    text_content: String,
}


#[tracing::instrument(
    name = "Publish a newsletter issue",
    skip(body, pool, email_client, user_id),
    fields(user_id=%*user_id)
)]
pub async fn publish_newsletter<T>(
    body: web::Form<FormData>,
    pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient<T>>,
    user_id: ReqData<UserId>,
) -> Result<HttpResponse, actix_web::Error>
where
    T: AsyncTransport + Send + Sync,
    <T as AsyncTransport>::Error: 'static + Send + Sync,
    <T as AsyncTransport>::Error: std::error::Error,
{
    let subscribers = get_confirmed_subscribers(&pool).await.map_err(e500)?;
    for subscriber in subscribers {
        match subscriber {
            Ok(subscriber) => {
                email_client
                    .send_email(
                        &subscriber.email,
                        body.title.clone(),
                        body.html_content.clone(),
                        body.text_content.clone(),
                    )
                    .await
                    .with_context(|| format!("Failed to send newsletter issue to {}", subscriber.email))
                    .map_err(e500)?;
            },
            Err(error) => {
                tracing::warn!(
                    error.cause_chain = ?error,
                    "Skipping a confirmed subscriber. \n\
                    Their stored contact details are invalid."
                );
            },
        }
    }

    FlashMessage::info("The newsletter issue has been published!").send();
    Ok(see_other("/admin/newsletters"))
}

struct ConfirmedSubscriber {
    email: SubscriberEmail,
}

impl std::fmt::Display for SubscriberEmail {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[tracing::instrument(name = "Get confirmed subscribers", skip(pool))]
async fn get_confirmed_subscribers(
    pool: &PgPool,
) -> Result<Vec<Result<ConfirmedSubscriber, anyhow::Error>>, anyhow::Error> {
    let rows = sqlx::query!(
        r#"
        SELECT email
        FROM subscriptions
        WHERE status = 'confirmed'
        "#
    )
    .fetch_all(pool)
    .await?;

    let confirmed_subscribers = rows
        .into_iter()
        .map(|r| match SubscriberEmail::parse(r.email) {
            Ok(email) => Ok(ConfirmedSubscriber { email }),
            Err(error) => Err(anyhow::anyhow!(error)),
        })
        .collect();
    Ok(confirmed_subscribers)
}
