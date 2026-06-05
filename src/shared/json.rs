use axum::{
    extract::rejection::JsonRejection,
    extract::{FromRequest, Request},
    Json,
};
use serde::de::DeserializeOwned;

use crate::shared::error::AppError;

pub struct AppJson<T>(pub T);

impl<S, T> FromRequest<S> for AppJson<T>
where
    S: Send + Sync,
    T: DeserializeOwned,
{
    type Rejection = AppError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        match Json::<T>::from_request(req, state).await {
            Ok(Json(value)) => Ok(Self(value)),
            Err(rejection) => Err(map_json_rejection(rejection)),
        }
    }
}

fn map_json_rejection(rejection: JsonRejection) -> AppError {
    match rejection {
        JsonRejection::MissingJsonContentType(_) => {
            AppError::BadRequest("content-type must be application/json".into())
        }
        other => AppError::BadRequest(other.body_text()),
    }
}
