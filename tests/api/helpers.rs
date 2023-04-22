use std::sync::Arc;

use argon2::password_hash::SaltString;
use argon2::Algorithm::Argon2id;
use argon2::{Argon2, Params, PasswordHasher, Version};
use mail_parser::Message;
use once_cell::sync::Lazy;
use reqwest::Url;
use sqlx::{Connection, Executor, PgConnection, PgPool};
use uuid::Uuid;

use zero2prod::configuration::{get_configuration, DatabaseSettings, Settings};
use zero2prod::domain::SubscriberName;
use zero2prod::email_client::{
    create_email_client_stub_that_accepts_all_messages, EmailClient, SenderInfo, StubMailTransport,
};
use zero2prod::startup::{ApplicationBuilder, ApplicationData};
use zero2prod::telemetry::{get_subscriber, init_subscriber};

static TRACING: Lazy<()> = Lazy::new(|| {
    let subscriber_name = "test".to_string();
    let default_log_level = "debug".to_string();
    if std::env::var("TEST_LOG").is_ok() {
        let subscriber = get_subscriber(subscriber_name, default_log_level, std::io::stdout);
        init_subscriber(subscriber);
    } else {
        let subscriber = get_subscriber(subscriber_name, default_log_level, std::io::sink);
        init_subscriber(subscriber);
    };
});

pub struct TestApp {
    pub address: String,
    pub db_pool: PgPool,
    pub port: u16,
    pub email_client: Arc<EmailClient<StubMailTransport>>,
    pub test_user: TestUser,
    pub api_client: reqwest::Client,
}

pub struct ConfirmationLinks {
    pub html: Url,
    pub plain_text: Url,
}

impl TestApp {
    pub async fn post_subscription(&self, body: String) -> reqwest::Response {
        self.api_client
            .post(format!("{}/subscriptions", &self.address))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .expect("Failed to execute request")
    }

    pub async fn get_confirmation_links(&self, transport: &StubMailTransport) -> ConfirmationLinks {
        let get_link = |s: &str| {
            let links: Vec<_> = linkify::LinkFinder::new()
                .links(s)
                .filter(|l| *l.kind() == linkify::LinkKind::Url)
                .collect();
            assert_eq!(links.len(), 1);
            let raw_link = links[0].as_str().to_owned();
            let mut confirmation_link = Url::parse(&raw_link).unwrap();
            assert_eq!(confirmation_link.host_str().unwrap(), "127.0.0.1");
            confirmation_link.set_port(Some(self.port)).unwrap();
            confirmation_link
        };

        let received_messages = transport.messages().await;
        let raw_message = received_messages[0].1.to_owned().into_bytes();
        let message = Message::parse(&raw_message).unwrap();
        let plain_text = get_link(&message.body_html(0).unwrap());
        let html = get_link(&message.body_text(0).unwrap());
        ConfirmationLinks { html, plain_text }
    }

    pub async fn post_newsletters<Body>(&self, body: &Body) -> reqwest::Response
    where Body: serde::Serialize {
        self.api_client
            .post(&format!("{}/admin/newsletters", &self.address))
            .form(body)
            .send()
            .await
            .expect("Failed to execute request")
    }

    pub async fn get_publish_newsletter(&self) -> reqwest::Response {
        self.api_client
            .get(&format!("{}/admin/newsletters", &self.address))
            .send()
            .await
            .expect("Failed to execute request")
    }

    pub async fn get_publish_newsletter_html(&self) -> String {
        self.get_publish_newsletter().await.text().await.unwrap()
    }

    pub async fn post_login<Body>(&self, body: &Body) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        self.api_client
            .post(&format!("{}/login", &self.address))
            .form(body)
            .send()
            .await
            .expect("Failed to execute request")
    }
    pub async fn get_login_html(&self) -> String {
        self.api_client
            .get(format!("{}/login", &self.address))
            .send()
            .await
            .expect("Failed to execute request")
            .text()
            .await
            .unwrap()
    }

    pub async fn get_admin_dashboard(&self) -> reqwest::Response {
        self.api_client
            .get(format!("{}/admin/dashboard", &self.address))
            .send()
            .await
            .expect("Failed to execute request")
    }
    pub async fn get_admin_dashboard_html(&self) -> String {
        self.get_admin_dashboard().await.text().await.unwrap()
    }

    pub async fn get_change_password(&self) -> reqwest::Response {
        self.api_client
            .get(&format!("{}/admin/password", &self.address))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn post_change_password<Body>(&self, body: &Body) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        self.api_client
            .post(&format!("{}/admin/password", &self.address))
            .form(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_change_password_html(&self) -> String {
        self.get_change_password().await.text().await.unwrap()
    }

    pub async fn post_logout(&self) -> reqwest::Response {
        self.api_client
            .post(&format!("{}/admin/logout", &self.address))
            .send()
            .await
            .expect("Failed to execute request.")
    }
}

pub struct TestAppConfiguration {
    pub email_client: Arc<EmailClient<StubMailTransport>>,
    pub configuration: Settings,
}

impl TestAppConfiguration {
    pub fn new() -> TestAppConfiguration {
        let configuration = get_configuration().expect("Failed to read configuration");
        let sender = configuration.email_client.sender().unwrap();
        let sender = SenderInfo(SubscriberName::parse("test".into()).unwrap(), sender);

        TestAppConfiguration {
            email_client: Arc::new(create_email_client_stub_that_accepts_all_messages(sender)),
            configuration,
        }
    }

    /*    pub fn set_email_client(&mut self, email_client: Arc<EmailClient<AsyncStubTransport>>) {
            self.email_client = email_client;
        }
    */
    pub fn get_configuration(&self) -> Settings {
        self.configuration.clone()
    }

    pub fn get_email_client(&self) -> Arc<EmailClient<StubMailTransport>> {
        self.email_client.clone()
    }
}

pub struct TestUser {
    pub user_id: Uuid,
    pub username: String,
    pub password: String,
}

impl TestUser {
    pub fn generate() -> Self {
        Self {
            user_id: Uuid::new_v4(),
            username: Uuid::new_v4().to_string(),
            password: Uuid::new_v4().to_string(),
            // password: "everythingstartssomewhere".into(),
        }
    }

    async fn store(&self, pool: &PgPool) {
        let salt = SaltString::generate(&mut rand::thread_rng());
        let password_hash = Argon2::new(Argon2id, Version::V0x13, Params::new(4096, 2, 1, None).unwrap())
            .hash_password(self.password.as_bytes(), &salt)
            .unwrap()
            .to_string();

        sqlx::query!(
            "INSERT INTO users(user_id, username, password_hash)
            VALUES ($1, $2, $3)",
            self.user_id,
            self.username,
            password_hash
        )
        .execute(pool)
        .await
        .expect("Failed to store test user.");
    }
}

pub async fn spawn_app(test_app_configuration: TestAppConfiguration) -> TestApp {
    Lazy::force(&TRACING);

    let configuration = {
        let mut c = test_app_configuration.get_configuration();
        c.database.database_name = Uuid::new_v4().to_string();
        c.application.port = 0;
        c
    };

    let connection_pool = configure_database(&configuration.database).await;

    let email_client = test_app_configuration.get_email_client();
    let application = ApplicationBuilder::new(configuration)
        .store(ApplicationData::EmailClient, email_client.clone())
        .build::<StubMailTransport>()
        .await;

    let api_client = reqwest::Client::builder()
        .cookie_store(true)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();

    let application_port = application.port();
    let _ = tokio::spawn(application.run_until_stopped());

    let test_app = TestApp {
        address: format!("http://localhost:{}", application_port),
        port: application_port,
        db_pool: connection_pool,
        email_client,
        test_user: TestUser::generate(),
        api_client,
    };
    test_app.test_user.store(&test_app.db_pool).await;
    test_app
}

pub async fn configure_database(config: &DatabaseSettings) -> PgPool {
    let mut connection = PgConnection::connect_with(&config.without_db())
        .await
        .expect("Failed to connect to postgres");
    connection
        .execute(format!(r#"CREATE DATABASE "{}";"#, config.database_name).as_str())
        .await
        .expect("Failed to create database.");

    let connection_pool = PgPool::connect_with(config.with_db())
        .await
        .expect("Failed to connect to postgres.");

    sqlx::migrate!("./migrations")
        .run(&connection_pool)
        .await
        .expect("Failed to migrate the database");

    connection_pool
}

pub fn assert_is_redirect_to(response: &reqwest::Response, location: &str) {
    assert_eq!(response.status().as_u16(), 303);
    assert_eq!(response.headers().get("Location").unwrap(), location);
}
