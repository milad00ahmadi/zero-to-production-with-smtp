use actix_web::http::StatusCode;

use crate::helpers::{spawn_app, TestAppConfiguration};

#[tokio::test]
async fn subscribe_returns_a_200_for_valid_form_data() {
    let app = spawn_app(TestAppConfiguration::new()).await;

    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";

    let response = app.post_subscription(body.into()).await;

    assert_eq!(StatusCode::OK, response.status());
}

#[tokio::test]
async fn subscribe_persists_the_new_subscriber() {
    let app = spawn_app(TestAppConfiguration::new()).await;

    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";

    app.post_subscription(body.into()).await;

    let saved = sqlx::query!("SELECT email, name, status FROM subscriptions",)
        .fetch_one(&app.db_pool)
        .await
        .expect("Failed to fetch saved subscription");

    assert_eq!(saved.email, "ursula_le_guin@gmail.com");
    assert_eq!(saved.name, "le guin");
    assert_eq!(saved.status, "pending_confirmation");
}

#[tokio::test]
async fn subscribe_returns_a_400_when_data_is_missing() {
    let app = spawn_app(TestAppConfiguration::new()).await;

    let test_cases = vec![
        ("name=le%20guin", "missing the email"),
        ("email=ursula_le_guin%40gmail.com", "missing the name"),
        ("", "missing both name and email"),
    ];

    for (invalid_body, error_message) in test_cases {
        let response = app.post_subscription(invalid_body.into()).await;
        assert_eq!(
            StatusCode::BAD_REQUEST,
            response.status(),
            "The api did not fail with 400 Bad Request when payload was {}.",
            error_message
        );
    }
}

#[tokio::test]
async fn subscribe_returns_a_400_when_fields_are_present_but_invalid() {
    let app = spawn_app(TestAppConfiguration::new()).await;
    let test_cases = vec![
        ("name=&email=ursula_le_guin%40gmail.com", "empty name"),
        ("name=Ursula&email=", "empty email"),
        ("name=Ursula&email=definitely-not-an-email", "invalid email"),
    ];

    for (body, description) in test_cases {
        let response = app.post_subscription(body.into()).await;

        assert_eq!(
            400,
            response.status().as_u16(),
            "The api did not return a 400 Bad Request when the payload was {}",
            description
        )
    }
}

#[tokio::test]
async fn subscribe_sends_a_confirmation_email_for_valid_data() {
    let configuration = TestAppConfiguration::new();
    let app = spawn_app(configuration).await;
    let email_client = app.email_client.clone();
    let body = "name=milad&email=uadrula_le_guin%40gmail.com";
    let response = app.post_subscription(body.into()).await;
    assert_eq!(200, response.status().as_u16());
    assert_eq!(email_client.get_transport_ref().messages().await.len(), 1);
}

#[tokio::test]
async fn subscribe_sends_a_confirmation_email_with_a_link() {
    let configuration = TestAppConfiguration::new();
    let app = spawn_app(configuration).await;
    let email_client = app.email_client.clone();
    let body = "name=milad&email=uadrula_le_guin%40gmail.com";
    let response = app.post_subscription(body.into()).await;

    assert_eq!(email_client.get_transport_ref().messages().await.len(), 1);
    let transport = email_client.get_transport_ref();
    let confirmation_links = app.get_confirmation_links(transport).await;
    assert_eq!(confirmation_links.html, confirmation_links.plain_text);
    assert_eq!(200, response.status().as_u16());
}

#[tokio::test]
async fn subscribe_fails_if_there_is_a_fatal_database_error() {
    let configuration = TestAppConfiguration::new();
    let app = spawn_app(configuration).await;
    let body = "name=milad&email=uadrula_le_guin%40gmail.com";

    sqlx::query!("ALTER TABLE subscription_tokens DROP COLUMN subscription_token;")
        .execute(&app.db_pool)
        .await
        .unwrap();

    let response = app.post_subscription(body.into()).await;

    assert_eq!(response.status().as_u16(), 500);
}
