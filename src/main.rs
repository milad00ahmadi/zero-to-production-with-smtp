use zero2prod::configuration::get_configuration;
use zero2prod::email_client::MailTransport;

use zero2prod::startup::ApplicationBuilder;
use zero2prod::telemetry::{get_subscriber, init_subscriber};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let subscriber = get_subscriber("zero2prod".into(), "info".into(), std::io::stdout);
    init_subscriber(subscriber);

    let configuration = get_configuration().expect("Failed to read configuration.");
    let application = ApplicationBuilder::new(configuration)
        .set_email_client_from_configuration()
        .build::<MailTransport>()
        .await;
    application.run_until_stopped().await?;
    Ok(())
}
