use actix_web::http::StatusCode;

use crate::helpers::{spawn_app, TestApp, TestAppConfiguration};

#[tokio::test]
async fn confirmation_without_tokens_are_rejected_with_a_400() {
    let configuration = TestAppConfiguration::new();
    let app = spawn_app(configuration).await;

    let response = reqwest::get(&format!("{}/subscriptions/confirm", app.address))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn the_link_returned_by_subscribe_returns_a_200_if_called() {
    let configuration = TestAppConfiguration::new();
    let app: TestApp = spawn_app(configuration).await;
    let email_client = app.email_client.clone();

    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";

    app.post_subscription(body.into()).await;

    let transport_ref = email_client.get_transport_ref();
    let confirmation_links = app.get_confirmation_links(transport_ref).await;
    let response = reqwest::get(confirmation_links.html).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn click_on_the_confirmation_link_confirms_a_subscriber() {
    let configuration = TestAppConfiguration::new();
    let app: TestApp = spawn_app(configuration).await;
    let email_client = app.email_client.clone();

    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";

    app.post_subscription(body.into()).await;

    let transport_ref = email_client.get_transport_ref();
    let confirmation_links = app.get_confirmation_links(transport_ref).await;
    let response = reqwest::get(confirmation_links.html).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let saved = sqlx::query!("SELECT email, name, status FROM subscriptions")
        .fetch_one(&app.db_pool)
        .await
        .expect("Failed to fetch saved subscriptions.");

    assert_eq!(saved.email, "ursula_le_guin@gmail.com");
    assert_eq!(saved.name, "le guin");
    assert_eq!(saved.status, "confirmed")
}
