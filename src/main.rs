use std::fmt::{Debug, Display};

use tokio::task::JoinError;
use zero2prod::configuration::get_configuration;
use zero2prod::email_client::MailTransport;

use zero2prod::issue_delivery_worker::run_worker_until_stopped;
use zero2prod::startup::ApplicationBuilder;
use zero2prod::telemetry::{get_subscriber, init_subscriber};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let subscriber = get_subscriber("zero2prod".into(), "info".into(), std::io::stdout);
    init_subscriber(subscriber);

    let configuration = get_configuration().expect("Failed to read configuration.");
    let application = ApplicationBuilder::new(configuration.clone())
        .set_email_client_from_configuration()
        .build::<MailTransport>()
        .await;
    let application = tokio::spawn(application.run_until_stopped());
    let worker = tokio::spawn(run_worker_until_stopped(configuration));
    tokio::select! {
        o = application => report_exit("API", o),
        o = worker => report_exit("Background worker", o)
    };
    Ok(())
}

fn report_exit(task_name: &str, outcome: Result<Result<(), impl Debug + Display>, JoinError>) {
    match outcome {
        Ok(Ok(())) => {
            tracing::info!("{} has exited", task_name)
        },
        Ok(Err(e)) => {
            tracing::error!(
            error.cause_chain = ?e,
            error.message = %e,
            "{} failed",
            task_name
            )
        },
        Err(e) => {
            tracing::error!(
            error.cause_chain = ?e,
            error.message = %e,
            "{}' task failed to complete",
            task_name
            )
        },
    }
}
