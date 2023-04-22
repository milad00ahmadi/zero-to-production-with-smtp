use std::future::{ready, Ready};
use std::rc::Rc;

use crate::session_state::{AuthenticatedUser, TypedSession};
use crate::utils::see_other;
use actix_session::SessionExt;
use actix_web::{
    body::EitherBody,
    dev::{self, Service, ServiceRequest, ServiceResponse, Transform},
    Error, HttpMessage,
};
use actix_web::{http, HttpResponse};
use futures_util::future::LocalBoxFuture;

pub struct Auth;

impl<S, B> Transform<S, ServiceRequest> for Auth
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Transform = AuthMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(AuthMiddleware {
            service: Rc::new(service),
        }))
    }
}

pub struct AuthMiddleware<S> {
    service: Rc<S>,
}

impl<S, B> Service<ServiceRequest> for AuthMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    dev::forward_ready!(service);
    fn call(&self, req: ServiceRequest) -> Self::Future {
        let srv = self.service.clone();
        let typed_session = TypedSession(req.get_session());
        req.extensions_mut().insert(typed_session);
        if req.path().contains("/admin/") == false {
            let res = srv.call(req);
            return Box::pin(async move { res.await.map(ServiceResponse::map_into_left_body) });
        }
        Box::pin(async move {
            let authenticated_user = AuthenticatedUser::extract(&req.extensions());
            if let Err(_) = authenticated_user {
                let (request, _pl) = req.into_parts();
                let response = see_other("/login").map_into_right_body();
                return Ok(ServiceResponse::new(request, response));
            }
            srv.call(req).await.map(ServiceResponse::map_into_left_body)
        })
    }
}
