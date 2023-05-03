use std::fmt::Write;

use actix_web::{http::header::ContentType, HttpResponse};
use actix_web_flash_messages::IncomingFlashMessages;

pub async fn publish_newsletter_form(flash_messages: IncomingFlashMessages) -> Result<HttpResponse, actix_web::Error> {
    let mut msg_html = String::new();
    for m in flash_messages.iter() {
        writeln!(msg_html, "<p><i>{}</i></p>", m.content()).unwrap();
    }
    let idempotency_key = uuid::Uuid::new_v4().to_string();
    let html_page = include_str!("newsletter.html")
        .replace("{msg_html}", &msg_html)
        .replace("{idempotency_key}", &idempotency_key);

    Ok(HttpResponse::Ok().content_type(ContentType::html()).body(html_page))
}
