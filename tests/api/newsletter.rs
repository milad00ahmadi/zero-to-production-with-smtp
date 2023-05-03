use fake::{
    faker::{internet::en::SafeEmail, name::en::Name},
    Fake,
};
use lettre::transport::stub::AsyncStubTransport;



use crate::helpers::{assert_is_redirect_to, spawn_app, ConfirmationLinks, TestApp, TestAppConfiguration};

async fn create_unconfirmed_subscribers(app: &TestApp) -> ConfirmationLinks {
    let name: String = Name().fake();
    let email: String = SafeEmail().fake();
    let body = serde_urlencoded::to_string(serde_json::json!({
        "name": name,
        "email": email
    }))
    .unwrap();

    let transport = app.email_client.get_transport_ref();
    let received_messages_count_before_sending_subscribe_request = transport.messages().await.len();
    let request = app.post_subscription(body).await.error_for_status().unwrap();

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

struct AsyncStubTransportSpy<'a> {
    transport_ref: &'a AsyncStubTransport,
    received_messages_before_assert: usize,
    expect: usize,
}

impl<'a> AsyncStubTransportSpy<'a> {
    pub async fn new(transport_ref: &'a AsyncStubTransport) -> AsyncStubTransportSpy {
        let recieved_messages_count = transport_ref.messages().await.len();
        Self {
            transport_ref,
            received_messages_before_assert: recieved_messages_count,
            expect: 0,
        }
    }

    pub fn expect(mut self, expect: usize) -> Self {
        self.expect = expect;
        self
    }

    pub async fn assert(&self) {
        let received_messages = self.transport_ref.messages().await.len();
        assert_eq!(self.received_messages_before_assert + self.expect, received_messages);
    }
}

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
        "html_content": "<p>Newsletter body as HTML</p>",
        "idempotency_key": uuid::Uuid::new_v4().to_string()
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
        "html_content": "<p>Newsletter body as HTML</p>",
        "idempotency_key": uuid::Uuid::new_v4().to_string()
    });

    let transport = app.email_client.get_transport_ref();
    let spy = AsyncStubTransportSpy::new(transport).await.expect(0);

    let response = app.post_newsletters(&newsletter_request_body).await;

    assert_is_redirect_to(&response, "/admin/newsletters");

    let html_page = app.get_publish_newsletter_html().await;
    assert!(html_page.contains(
        "<p><i>The newsletter issue has been accepted - \
        emails will go out shortly.</i></p>"
    ));

    app.dispatch_all_pending_emails().await;
    spy.assert().await;
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
        "html_content": "<p>Newsletter body as HTML</p>",
        "idempotency_key": uuid::Uuid::new_v4().to_string()
    });
    let transport = app.email_client.get_transport_ref();
    let spy = AsyncStubTransportSpy::new(transport).await.expect(1);

    let response = app.post_newsletters(&newsletter_request_body).await;

    assert_is_redirect_to(&response, "/admin/newsletters");

    let html_page = app.get_publish_newsletter_html().await;
    assert!(html_page.contains(
        "<p><i>The newsletter issue has been accepted - \
        emails will go out shortly.</i></p>"
    ));

    app.dispatch_all_pending_emails().await;
    spy.assert().await;
}

#[tokio::test]
async fn newsletter_creation_is_idempotent() {
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
        "html_content": "<p>Newsletter body as HTML</p>",
        "idempotency_key": uuid::Uuid::new_v4().to_string()
    });
    let transport = app.email_client.get_transport_ref();
    let spy = AsyncStubTransportSpy::new(transport).await.expect(1);

    let response = app.post_newsletters(&newsletter_request_body.clone()).await;
    assert_is_redirect_to(&response, "/admin/newsletters");
    let html_page = app.get_publish_newsletter_html().await;
    assert!(html_page.contains(
        "<p><i>The newsletter issue has been accepted - \
        emails will go out shortly.</i></p>"
    ));

    let response = app.post_newsletters(&newsletter_request_body.clone()).await;
    assert_is_redirect_to(&response, "/admin/newsletters");
    let html_page = app.get_publish_newsletter_html().await;
    assert!(html_page.contains(
        "<p><i>The newsletter issue has been accepted - \
        emails will go out shortly.</i></p>"
    ));

    app.dispatch_all_pending_emails().await;
    spy.assert().await;
}

#[tokio::test]
async fn concurrent_form_submission_is_handled_gracefully() {
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
        "html_content": "<p>Newsletter body as HTML</p>",
        "idempotency_key": uuid::Uuid::new_v4().to_string()
    });
    let transport = app.email_client.get_transport_ref();
    let spy = AsyncStubTransportSpy::new(transport).await.expect(1);

    let response1 = app.post_newsletters(&newsletter_request_body);
    let response2 = app.post_newsletters(&newsletter_request_body);
    let (response1, response2) = tokio::join!(response1, response2);
    assert_eq!(response1.status(), response2.status());
    assert_eq!(response1.text().await.unwrap(), response2.text().await.unwrap());

    app.dispatch_all_pending_emails().await;
    spy.assert().await;
}

// TODO
// #[tokio::test]
// async fn transient_errors_do_not_cause_duplicate_deliveries_on_retries() {
//     let configuration = TestAppConfiguration::new();
//     let app = spawn_app(configuration).await;
//     create_confirmed_subscriber(&app).await;
//     create_confirmed_subscriber(&app).await;
//     app.test_user.login(&app).await;
//     todo!()
// }