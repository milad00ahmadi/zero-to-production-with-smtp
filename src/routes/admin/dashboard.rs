use crate::session_state::{AuthenticatedUser, TypedSession};
use crate::utils::e500;
use actix_web::http::header::{ContentType, LOCATION};
use actix_web::web::ReqData;
use actix_web::{web, HttpResponse};
use anyhow::Context;
use sqlx::PgPool;
use uuid::Uuid;

pub async fn admin_dashboard(
    pool: web::Data<PgPool>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, actix_web::Error> {
    // let username = if let Some(user_id) = session.get_user_id().map_err(e500)? {
    //     get_username(user_id, &pool).await.map_err(e500)?
    // } else {
    //     return Ok(HttpResponse::SeeOther().insert_header((LOCATION, "/login")).finish());
    // };
    let user_id = user.id().map_err(e500)?;
    let username = get_username(user_id, &pool).await.map_err(e500)?;
    Ok(HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(include_str!("dashboard.html").replace("{username}", &username)))
}

pub async fn get_username(user_id: Uuid, pool: &PgPool) -> Result<String, anyhow::Error> {
    let row = sqlx::query!(
        r#"SELECT username
        FROM users
        WHERE user_id = $1"#,
        user_id
    )
    .fetch_one(pool)
    .await
    .context("Failed to perform a query to retrieve a username.")?;

    Ok(row.username)
}
