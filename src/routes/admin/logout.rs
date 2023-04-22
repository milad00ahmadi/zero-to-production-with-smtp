use actix_web::HttpResponse;
use actix_web_flash_messages::FlashMessage;

use crate::{session_state::{TypedSession}, utils::see_other};

pub async fn log_out(typed_session: TypedSession) -> Result<HttpResponse, actix_web::Error> {
    typed_session.logout();
    FlashMessage::info("You have successfully logged out.").send();
    Ok(see_other("/login"))
}
