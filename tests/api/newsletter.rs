use sqlx::PgPool;
use uuid::Uuid;

use crate::helpers::{assert_is_redirect_to, spawn_app, ConfirmationLinks, TestApp, TestAppConfiguration};

#[tokio::test]
async fn you_must_be_logged_in_to_see_the_newsletter_form() {
    let configuration = TestAppConfiguration::new();
    let app = spawn_app(configuration).await;

    let response = app.get_publish_newsletter().await;
    assert_is_redirect_to(&response, "/login");
}

#[tokio::test]
async fn you_must_be_logged_in_to_publish_newsletter() {
    let configuration = TestAppConfiguration::new();
    let app = spawn_app(configuration).await;

    let newsletter_request_body = serde_json::json!({
        "title": "Newsletter title",
        "text_content": "Newsletter body as plain text",
        "html_content": "<p>Newsletter body as HTML</p>"
    });
    let response = app.post_newsletters(&newsletter_request_body).await;
    assert_is_redirect_to(&response, "/login");
}

#[tokio::test]
async fn newsletters_are_not_delivered_to_unconfirmed_subscribers() {
    let configuration = TestAppConfiguration::new();
    let app = spawn_app(configuration).await;
    create_unconfirmed_subscribers(&app).await;

    app.post_login(&serde_json::json!({
        "username": &app.test_user.username,
        "password": &app.test_user.password
    }))
    .await;

    let newsletter_request_body = serde_json::json!({
        "title": "Newsletter title",
        "text_content": "Newsletter body as plain text",
        "html_content": "<p>Newsletter body as HTML</p>"
    });

    let transport = app.email_client.get_transport_ref();
    let received_messages_count_before_sending_subscribe_request = transport.messages().await.len();

    let response = app.post_newsletters(&newsletter_request_body).await;

    let received_messages_count_after_sending_subscriber_request = transport.messages().await.len();

    assert_eq!(
        received_messages_count_before_sending_subscribe_request,
        received_messages_count_after_sending_subscriber_request
    );

    assert_is_redirect_to(&response, "/admin/newsletters");

    let html_page = app.get_publish_newsletter_html().await;
    assert!(html_page.contains("<p><i>The newsletter issue has been published!</i></p>"));
}

#[tokio::test]
async fn newsletters_are_delivered_to_confirmed_subscribers() {
    let configuration = TestAppConfiguration::new();
    let app = spawn_app(configuration).await;
    create_confirmed_subscriber(&app).await;

    app.post_login(&serde_json::json!({
        "username": &app.test_user.username,
        "password": &app.test_user.password
    }))
    .await;

    let newsletter_request_body = serde_json::json!({
        "title": "Newsletter title",
        "text_content": "Newsletter body as plain text",
        "html_content": "<p>Newsletter body as HTML</p>"
    });
    let transport = app.email_client.get_transport_ref();
    let received_messages_count_before_sending_subscribe_request = transport.messages().await.len();

    let response = app.post_newsletters(&newsletter_request_body).await;

    let received_messages_count_after_sending_subscriber_request = transport.messages().await.len();
    assert_eq!(
        received_messages_count_before_sending_subscribe_request + 1,
        received_messages_count_after_sending_subscriber_request
    );
    assert_is_redirect_to(&response, "/admin/newsletters");

    let html_page = app.get_publish_newsletter_html().await;
    assert!(html_page.contains("<p><i>The newsletter issue has been published!</i></p>"));
}

async fn create_unconfirmed_subscribers(app: &TestApp) -> ConfirmationLinks {
    let body = "name=milad&email=milad%40gmail.com";

    let transport = app.email_client.get_transport_ref();
    let received_messages_count_before_sending_subscribe_request = transport.messages().await.len();
    let request = app.post_subscription(body.into()).await.error_for_status().unwrap();

    let received_messages_count_after_sending_subscriber_request = transport.messages().await.len();
    assert_eq!(
        received_messages_count_before_sending_subscribe_request + 1,
        received_messages_count_after_sending_subscriber_request
    );

    assert_eq!(request.status(), 200);
    app.get_confirmation_links(transport).await
}

async fn create_confirmed_subscriber(app: &TestApp) {
    let confirmation_link = create_unconfirmed_subscribers(app).await;

    reqwest::get(confirmation_link.html)
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
}
