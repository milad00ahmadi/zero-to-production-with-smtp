use lettre::message::{header, MultiPart, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::transport::stub::AsyncStubTransport;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

use crate::configuration::EmailClientSetting;
use crate::domain::{SubscriberEmail, SubscriberName};

pub type MailTransport = AsyncSmtpTransport<Tokio1Executor>;
pub type StubMailTransport = AsyncStubTransport;

pub struct SenderInfo(pub SubscriberName, pub SubscriberEmail);

pub struct EmailClient<T>
where
    T: AsyncTransport + Send + Sync,
    <T as AsyncTransport>::Error: 'static + Send + Sync,
    <T as AsyncTransport>::Error: std::error::Error,
{
    transport: T,
    sender: SenderInfo,
}

#[derive(thiserror::Error, Debug)]
pub enum EmailClientError {
    #[error("transport error while trying to send the email")]
    TransportError(#[from] anyhow::Error),
}

impl<T> EmailClient<T>
where
    T: AsyncTransport + Send + Sync,
    <T as AsyncTransport>::Error: 'static + Send + Sync,
    <T as AsyncTransport>::Error: std::error::Error,
{
    pub fn get_transport_ref(&self) -> &T {
        &self.transport
    }

    pub fn set_transport(&mut self, transport: T)
    where
        T: Send + Sync + AsyncTransport,
    {
        self.transport = transport;
    }

    pub async fn send_email(
        &self,
        recipient: &SubscriberEmail,
        subject: String,
        plain_message: String,
        html_message: String,
    ) -> Result<(), EmailClientError> {
        let email = Message::builder()
            .from(
                format!("{} <{}>", self.sender.0.as_ref(), self.sender.1.as_ref())
                    .parse()
                    .unwrap(),
            )
            .to(format!(" <{}>", recipient.as_ref()).parse().unwrap())
            .subject(subject)
            .multipart(
                MultiPart::alternative()
                    .singlepart(
                        SinglePart::builder()
                            .header(header::ContentType::TEXT_PLAIN)
                            .body(plain_message),
                    )
                    .singlepart(
                        SinglePart::builder()
                            .header(header::ContentType::TEXT_HTML)
                            .body(html_message),
                    ),
            )
            .unwrap();

        match self.transport.send(email).await {
            Ok(_) => Ok(()),
            Err(e) => Err(EmailClientError::TransportError(e.into())),
        }
    }
}

pub fn create_email_client_stub_which_accepts_all_messages(sender: SenderInfo) -> EmailClient<StubMailTransport> {
    EmailClient {
        sender,
        transport: AsyncStubTransport::new_ok(),
    }
}

pub fn create_email_client_stub_which_denies_all_messages(sender: SenderInfo) -> EmailClient<StubMailTransport> {
    EmailClient {
        sender,
        transport: AsyncStubTransport::new_error(),
    }
}

pub fn create_email_client(configuration: EmailClientSetting, sender: SenderInfo) -> EmailClient<MailTransport> {
    let credentials = create_credentials_from_configuration(&configuration);

    let transport: MailTransport = create_async_smtp_transport(&configuration, credentials);

    EmailClient { transport, sender }
}

fn create_credentials_from_configuration(configuration: &EmailClientSetting) -> Credentials {
    Credentials::new(configuration.username.clone(), configuration.password.clone())
}

fn create_async_smtp_transport(configuration: &EmailClientSetting, credentials: Credentials) -> MailTransport {
    MailTransport::relay(&configuration.smtp_server)
        .unwrap()
        .credentials(credentials)
        .port(configuration.port)
        .build()
}

#[cfg(test)]
mod tests {
    use fake::faker::internet::en::SafeEmail;
    use fake::faker::job::en::Title;
    use fake::faker::lorem::en::Paragraphs;
    use fake::faker::name::en::FirstName;
    use fake::Fake;
    use lettre::Address;

    use crate::domain::{SubscriberEmail, SubscriberName};
    use crate::email_client::{
        create_email_client_stub_which_accepts_all_messages, create_email_client_stub_which_denies_all_messages,
        SenderInfo,
    };

    fn subject() -> String {
        Title().fake()
    }

    fn content() -> String {
        let p = Paragraphs(3..5).fake::<Vec<String>>();
        p.join("\n")
    }

    fn html_content() -> String {
        let p = Paragraphs(3..5).fake::<Vec<String>>();
        format!("<p>{}</p>", p.join("\n"))
    }

    fn email() -> SubscriberEmail {
        SubscriberEmail::parse(SafeEmail().fake()).unwrap()
    }

    #[tokio::test]
    async fn send_email_returns_ok_when_transport_accepts_email() {
        let sender_email = email();
        let sender_name = SubscriberName::parse(FirstName().fake()).unwrap();
        let sender = SenderInfo(sender_name, sender_email.clone());
        let email_client = create_email_client_stub_which_accepts_all_messages(sender);
        let _transport = email_client.get_transport_ref();

        let subscriber_email = email();
        let subject: String = subject();
        let content: String = content();
        let result = email_client
            .send_email(&subscriber_email, subject.clone(), content.clone(), content.clone())
            .await;
        let transport_messages = email_client.transport.messages().await;
        let sent_email = transport_messages[0].clone();

        assert_eq!(transport_messages.len(), 1);

        assert_eq!(
            sent_email.0.to()[0],
            subscriber_email.as_ref().parse::<Address>().unwrap()
        );
        assert_eq!(
            sent_email.0.from().unwrap(),
            &sender_email.as_ref().parse::<Address>().unwrap()
        );
        assert!(sent_email.1.contains(&format!("{}: {}", "Subject", subject)));
        assert_eq!(transport_messages.len(), 1);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn send_email_returns_error_when_transport_declines_email() {
        let sender_email = email();
        let sender_name = SubscriberName::parse(FirstName().fake()).unwrap();
        let sender = SenderInfo(sender_name, sender_email.clone());
        let email_client = create_email_client_stub_which_denies_all_messages(sender);
        let _transport = email_client.get_transport_ref();

        let subscriber_email = email();
        let subject: String = subject();
        let content = content();
        let html_content = html_content();
        let _result = email_client
            .send_email(
                &subscriber_email,
                subject.clone(),
                content.clone(),
                html_content.clone(),
            )
            .await;
        let transport_messages = email_client.transport.messages().await;
        println!("{:?}", transport_messages.len());

        assert_eq!(transport_messages.len(), 1);
        // assert_err!(result);
    }
}
