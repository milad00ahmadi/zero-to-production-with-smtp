use actix_session::storage::RedisSessionStore;
use actix_session::SessionMiddleware;
use actix_web_lab::middleware::from_fn;
use std::any::Any;
use std::collections::HashMap;
use std::net::TcpListener;
use std::sync::Arc;

use actix_web::cookie::Key;
use actix_web::dev::Server;
use actix_web::web::Data;
use actix_web::{web, App, HttpServer};
use actix_web_flash_messages::storage::CookieMessageStore;
use actix_web_flash_messages::FlashMessagesFramework;
use lettre::AsyncTransport;
use secrecy::{ExposeSecret, Secret};
use sqlx::PgPool;
use tracing_actix_web::TracingLogger;

use crate::authentication::reject_anonymous_users;
use crate::configuration::Settings;
use crate::domain::SubscriberName;
use crate::email_client::{EmailClient, SenderInfo};
use crate::routes::{
    admin_dashboard, change_password, change_password_form, confirm, health_check, home, log_out, login, login_form,
    publish_newsletter, subscribe,
};
use crate::{email_client};

pub struct Application {
    port: u16,
    server: Server,
}

impl Application {
    pub async fn build<T>(configuration: Settings, email_client: Arc<EmailClient<T>>) -> Result<Self, anyhow::Error>
    where
        T: 'static + AsyncTransport + Send + Sync,
        <T as AsyncTransport>::Error: 'static + Send + Sync,
        <T as AsyncTransport>::Error: std::error::Error,
    {
        let address = format!("{}:{}", configuration.application.host, configuration.application.port);
        let connection_pool = PgPool::connect_with(configuration.database.with_db())
            .await
            .expect("Failed to connect to Postgres");

        tracing::info!("listening on {}", &address);
        let listener = TcpListener::bind(address).expect("Failed to bind random port");
        let port = listener.local_addr().unwrap().port();

        let server = run::<T>(
            listener,
            connection_pool,
            email_client,
            configuration.application.base_url,
            configuration.application.hmac_secret,
            configuration.redis_uri,
        )
        .await?;

        Ok(Self { port, server })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub async fn run_until_stopped(self) -> Result<(), std::io::Error> {
        self.server.await
    }
}

#[derive(Clone)]
pub struct HmacSecret(pub Secret<String>);

#[derive(Eq, PartialEq, Hash)]
pub enum ApplicationData {
    EmailClient,
}

pub struct ApplicationBuilder {
    configuration: Settings,
    items: HashMap<ApplicationData, Arc<dyn Any + Send + Sync>>,
}

impl ApplicationBuilder {
    pub fn new(configuration: Settings) -> Self {
        ApplicationBuilder {
            configuration,
            items: HashMap::new(),
        }
    }

    pub fn store<T: Any + Send + Sync + 'static>(mut self, key: ApplicationData, email_client: Arc<T>) -> Self {
        self.items.insert(key, email_client);
        self
    }

    pub fn set_email_client_from_configuration(self) -> Self {
        let sender_email = self.configuration.email_client.sender().unwrap();
        let sender_name = SubscriberName::parse(self.configuration.email_client.name.clone()).unwrap();
        let sender = SenderInfo(sender_name, sender_email);
        let email_client = email_client::create_email_client(self.configuration.email_client.clone(), sender);
        self.store(ApplicationData::EmailClient, Arc::new(email_client))
    }

    fn get_item<T: Send + Sync + 'static>(&mut self, key: ApplicationData) -> Arc<T> {
        if let Some(client) = self.items.remove(&key) {
            client.downcast::<T>().unwrap()
        } else {
            panic!("Please provide a email client for the application builder")
        }
    }

    pub async fn build<T>(mut self) -> Application
    where
        T: 'static + AsyncTransport + Send + Sync,
        <T as AsyncTransport>::Error: 'static + Send + Sync,
        <T as AsyncTransport>::Error: std::error::Error,
    {
        let email_client = self.get_item::<EmailClient<T>>(ApplicationData::EmailClient);

        Application::build(self.configuration, email_client)
            .await
            .expect("Failed to build the application")
    }
}

pub struct ApplicationBaseUrl(pub String);

pub async fn run<T>(
    listener: TcpListener,
    db_pool: PgPool,
    email_client: Arc<EmailClient<T>>,
    base_url: String,
    hmac_secret: Secret<String>,
    redis_uri: Secret<String>,
) -> Result<Server, anyhow::Error>
where
    T: 'static + AsyncTransport + Send + Sync,
    <T as AsyncTransport>::Error: 'static + Send + Sync,
    <T as AsyncTransport>::Error: std::error::Error,
{
    let connection = web::Data::new(db_pool);
    let email_client = Data::from(email_client);
    let base_url = Data::new(ApplicationBaseUrl(base_url));
    let secret_key = Key::from(hmac_secret.expose_secret().as_bytes());
    let message_store = CookieMessageStore::builder(secret_key.clone()).build();
    let message_framework = FlashMessagesFramework::builder(message_store).build();
    let redis_store = RedisSessionStore::new(redis_uri.expose_secret()).await?;
    let server = HttpServer::new(move || {
        App::new()
            .wrap(message_framework.clone())
            .wrap(SessionMiddleware::new(redis_store.clone(), secret_key.clone()))
            .wrap(TracingLogger::default())
            .route("/health_check", web::get().to(health_check))
            .route("/subscriptions", web::post().to(subscribe::<T>))
            .route("/subscriptions/confirm", web::get().to(confirm))
            .route("/newsletters", web::post().to(publish_newsletter::<T>))
            .route("/", web::get().to(home))
            .route("/login", web::get().to(login_form))
            .route("/login", web::post().to(login))
            .service(
                web::scope("/admin")
                    .wrap(from_fn(reject_anonymous_users))
                    .route("/dashboard", web::get().to(admin_dashboard))
                    .route("/password", web::get().to(change_password_form))
                    .route("/password", web::post().to(change_password))
                    .route("/logout", web::post().to(log_out)),
            )
            .app_data(connection.clone())
            .app_data(email_client.clone())
            .app_data(base_url.clone())
            .app_data(Data::new(HmacSecret(hmac_secret.clone())))
    })
    .listen(listener)?
    .run();
    Ok(server)
}
