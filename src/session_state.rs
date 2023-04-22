use actix_session::{Session, SessionExt, SessionGetError, SessionInsertError};
use actix_web::dev::{Extensions, Payload};
use actix_web::{Error, FromRequest, HttpMessage, HttpRequest, HttpResponse};
use anyhow::{anyhow, Context};
use std::future::{ready, Ready};
use actix_web::http::header::LOCATION;
use uuid::Uuid;

#[derive(Clone)]
pub struct AuthenticatedUser(TypedSession);

impl AuthenticatedUser {
    pub fn id(&self) -> Result<Uuid, SessionGetError> {
        self.0.get_user_id().map(|v| v.expect("cannot find user uuid"))
    }

    pub fn login(ext: &Extensions, user_id: Uuid) -> Result<Self, anyhow::Error> {
        let session = TypedSession::extract(ext);
        session.renew();
        session
            .insert_user_id(user_id)
            .context("cannot insert user id to redis backend")?;
        Ok(Self(session))
    }

    pub fn logout(self) {
        self.0.logout();
    }

    pub fn extract(ext: &Extensions) -> Result<Self, anyhow::Error> {
        let session = TypedSession::extract(ext);
        session.get_user_id()?.ok_or_else(|| anyhow!("user is not logged in"))?;
        Ok(Self(session))
    }
}

#[derive(Clone)]
pub struct TypedSession(pub Session);

impl TypedSession {
    const USER_ID_KEY: &'static str = "user_id";

    fn extract(ext: &Extensions) -> Self {
        ext.get::<Self>().expect("No TypedSession was found").to_owned()
    }

    pub fn renew(&self) {
        self.0.renew();
    }

    pub fn insert_user_id(&self, user_id: Uuid) -> Result<(), SessionInsertError> {
        self.0.insert(Self::USER_ID_KEY, user_id)
    }

    pub fn get_user_id(&self) -> Result<Option<Uuid>, SessionGetError> {
        self.0.get(Self::USER_ID_KEY)
    }

    pub fn logout(&self) {
        self.0.purge()
    }
}

impl FromRequest for AuthenticatedUser {
    type Error = actix_web::Error;

    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, payload: &mut Payload) -> Self::Future {
        ready(AuthenticatedUser::extract(&req.extensions()).map_err(|err| {
            actix_web::error::ErrorUnauthorized(err)
        }))
    }
}

// ---> actix does not convert request to struct fields
// impl FromRequest for TypedSession {
//     type Error = <Session as FromRequest>::Error;
//     type Future = Ready<Result<TypedSession, Self::Error>>;

//     fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
//         ready(Ok(TypedSession(req.get_session())))
//     }
// }
