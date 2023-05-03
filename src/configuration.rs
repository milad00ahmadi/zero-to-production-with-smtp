use crate::domain::SubscriberEmail;
use secrecy::{ExposeSecret, Secret};
use serde_aux::prelude::*;
use sqlx::postgres::PgConnectOptions;

#[derive(serde::Deserialize, Debug, Clone)]
pub struct Settings {
    pub database: DatabaseSettings,
    pub application: ApplicationSetting,
    pub email_client: EmailClientSetting,
    pub redis_uri: Secret<String>,
}

#[derive(serde::Deserialize, Debug, Clone)]
pub struct ApplicationSetting {
    pub host: String,
    pub base_url: String,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub port: u16,
    pub hmac_secret: Secret<String>,
}

#[derive(serde::Deserialize, Debug, Clone)]
pub struct EmailClientSetting {
    pub smtp_server: String,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub port: u16,
    pub name: String,
    pub username: String,
    pub password: String,
}

impl EmailClientSetting {
    pub fn sender(&self) -> Result<SubscriberEmail, String> {
        SubscriberEmail::parse(self.username.clone())
    }
}

#[derive(serde::Deserialize, Debug, Clone)]
pub struct DatabaseSettings {
    pub username: String,
    pub password: Secret<String>,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub port: u16,
    pub host: String,
    pub database_name: String,
}

impl DatabaseSettings {
    pub fn with_db(&self) -> PgConnectOptions {
        self.without_db().database(&self.database_name)
    }

    pub fn without_db(&self) -> PgConnectOptions {
        PgConnectOptions::new()
            .host(&self.host)
            .username(&self.username)
            .password(self.password.expose_secret())
            .port(self.port)
    }
}

pub fn get_configuration() -> Result<Settings, config::ConfigError> {
    let mut settings = config::Config::default();
    let base_path = std::env::current_dir().expect("Failed to determine current directory");
    let configuration_directory = base_path.join("configuration");

    settings.merge(config::File::from(configuration_directory.join("base")).required(true))?;

    let environment: Environment = std::env::var("APP_ENVIRONMENT")
        .unwrap_or("local".into())
        .try_into()
        .expect("failed to parse APP_ENVIRONMENT");

    settings.merge(
        config::File::from(configuration_directory.join(environment.as_str())).required(true),
    )?;

    settings.merge(config::Environment::with_prefix("app").separator("__"))?;

    settings.try_into()
}

pub enum Environment {
    Local,
    Production,
}

impl Environment {
    pub fn as_str(&self) -> &'static str {
        match self {
            Environment::Local => "local",
            Environment::Production => "production",
        }
    }
}

impl TryFrom<String> for Environment {
    type Error = String;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        match s.to_lowercase().as_str() {
            "local" => Ok(Self::Local),
            "production" => Ok(Self::Production),
            other => Err(format!(
                "{} is not a supported environment. Use either `local` or `production`.",
                other
            )),
        }
    }
}
