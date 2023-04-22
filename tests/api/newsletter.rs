use uuid::Uuid;

use crate::helpers::{spawn_app, ConfirmationLinks, TestApp, TestAppConfiguration};

#[tokio::test]
async fn newsletters_are_not_delivered_to_unconfirmed_subscribers() {
    let configuration = TestAppConfiguration::new();
    let app = spawn_app(configuration).await;
    create_unconfirmed_subscribers(&app).await;

    let newsletter_request_body = serde_json::json!({
        "title": "Newsletter title",
        "content": {
            "text": "Newsletter body as plain text",
            "html": "<p>Newsletter body as HTML</p>"
        }
    });

    let response = app.post_newsletters(newsletter_request_body).await;

    assert_eq!(response.status().as_u16(), 200);
}

#[tokio::test]
async fn newsletters_are_delivered_to_confirmed_subscribers() {
    let configuration = TestAppConfiguration::new();
    let app = spawn_app(configuration).await;
    create_confirmed_subscriber(&app).await;

    let newsletter_request_body = serde_json::json!({
        "title": "Newsletter title",
        "content": {
            "text": "Newsletter body as plain text",
            "html": "<p>Newsletter body as HTML</p>"
        }
    });
    let transport = app.email_client.get_transport_ref();
    let received_messages_count_before_sending_subscribe_request = transport.messages().await.len();

    let response = app.post_newsletters(newsletter_request_body).await;

    let received_messages_count_after_sending_subscriber_request = transport.messages().await.len();
    assert_eq!(
        received_messages_count_before_sending_subscribe_request + 1,
        received_messages_count_after_sending_subscriber_request
    );
    assert_eq!(response.status().as_u16(), 200);
}

#[tokio::test]
async fn newsletters_returns_400_for_invalid_data() {
    let configuration = TestAppConfiguration::new();
    let app = spawn_app(configuration).await;
    create_confirmed_subscriber(&app).await;

    let test_cases = vec![
        (
            serde_json::json!({
                "content": {
                    "text": "Newsletter body as plain text",
                    "html": "<p>Newsletter body as HTML</p>"
                },
            }),
            "missing title",
        ),
        (
            serde_json::json!({
                "title": "Newsletter title",
            }),
            "missing content",
        ),
    ];

    for (invalid_body, error_message) in test_cases {
        let response = app.post_newsletters(invalid_body).await;

        assert_eq!(
            400,
            response.status().as_u16(),
            "The Api did not fail with 400 Bad Request when payload was {}",
            error_message
        );
    }
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

#[tokio::test]
async fn requests_missing_authorization_are_rejected() {
    let configuration = TestAppConfiguration::new();
    let app = spawn_app(configuration).await;

    let response = reqwest::Client::new()
        .post(&format!("{}/newsletters", &app.address))
        .json(&serde_json::json!({
            "title": "newsletter title",
            "content": {
                "text": "Newsletter body as plain text",
                "html": "<p>Newsletter body as HTML</p>"
            },
        }))
        .send()
        .await
        .expect("Failed to execute request");

    assert_eq!(401, response.status().as_u16());
    assert_eq!(r#"Basic realm="publish""#, response.headers()["WWW-Authenticate"]);
}

#[tokio::test]
async fn non_existing_user_is_rejected() {
    let configuration = TestAppConfiguration::new();
    let app = spawn_app(configuration).await;

    let username = Uuid::new_v4().to_string();
    let password = Uuid::new_v4().to_string();

    let response = reqwest::Client::new()
        .post(format!("{}/newsletters", &app.address))
        .basic_auth(username, Some(password))
        .json(&serde_json::json!({
            "title": "newsletter title",
            "content": {
                "text": "Newsletter body as plain text",
                "html": "<p>Newsletter body as HTML</p>"
            },
        }))
        .send()
        .await
        .expect("Failed to execute request.");

    assert_eq!(401, response.status().as_u16());
    assert_eq!(r#"Basic realm="publish""#, response.headers()["WWW-Authenticate"])
}

#[tokio::test]
async fn invalid_password_is_rejected() {
    let configuration = TestAppConfiguration::new();
    let app = spawn_app(configuration).await;

    let username = &app.test_user.username;
    let password = Uuid::new_v4().to_string();
    assert_ne!(app.test_user.password, password);

    let response = reqwest::Client::new()
        .post(format!("{}/newsletters", &app.address))
        .basic_auth(username, Some(password))
        .json(&serde_json::json!({
            "title": "newsletter title",
            "content": {
                "text": "Newsletter body as plain text",
                "html": "<p>Newsletter body as HTML</p>"
            },
        }))
        .send()
        .await
        .expect("Failed to execute request.");

    assert_eq!(401, response.status().as_u16());
    assert_eq!(r#"Basic realm="publish""#, response.headers()["WWW-Authenticate"])
}
