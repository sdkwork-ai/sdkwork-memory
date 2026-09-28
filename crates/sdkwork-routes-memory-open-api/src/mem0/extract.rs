//! Extractors that reject in the mem0 dialect.
//!
//! The shared `MemoryJson` / `MemoryQuery` rejections render as
//! `application/problem+json`, which the mem0 clients cannot read (they look for
//! `detail`). These wrappers keep the one error shape the compatibility wire
//! promises, for malformed bodies and malformed query strings alike.

use axum::extract::rejection::{JsonRejection, QueryRejection};
use axum::extract::{FromRequest, FromRequestParts, Json, Query, Request};
use axum::http::request::Parts;
use serde::de::DeserializeOwned;

use crate::mem0::error::Mem0Error;

/// Strict JSON body extraction reporting a mem0 `detail`.
#[derive(Debug, Clone)]
pub struct Mem0Json<T>(pub T);

impl<T, S> FromRequest<S> for Mem0Json<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = Mem0Error;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let Json(value) = Json::<T>::from_request(req, state)
            .await
            .map_err(body_rejection)?;
        Ok(Self(value))
    }
}

/// Strict typed query extraction reporting a mem0 `detail`.
#[derive(Debug, Clone, Copy, Default)]
pub struct Mem0Query<T>(pub T);

impl<T, S> FromRequestParts<S> for Mem0Query<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = Mem0Error;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Query::<T>::try_from_uri(&parts.uri)
            .map(|Query(query)| Self(query))
            .map_err(|rejection: QueryRejection| {
                Mem0Error::invalid_parameter(format!(
                    "query parameters must use the canonical names and types: {rejection}"
                ))
            })
    }
}

fn body_rejection(rejection: JsonRejection) -> Mem0Error {
    Mem0Error::invalid_parameter(format!(
        "request body must be well-formed JSON matching the operation schema: {}",
        rejection.body_text()
    ))
}
