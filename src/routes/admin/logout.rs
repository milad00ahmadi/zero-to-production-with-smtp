use actix_web::HttpResponse;
use actix_web_flash_messages::FlashMessage;

use crate::{session_state::AuthenticatedUser, utils::see_other};

pub async fn log_out(user: AuthenticatedUser) -> Result<HttpResponse, actix_web::Error> {
    user.logout();
    FlashMessage::info("You have successfully logged out.").send();
    Ok(see_other("/login"))
}
