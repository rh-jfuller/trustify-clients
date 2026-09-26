//! Async bindings for the Trustify REST API.
//!
//! [`api`] exposes the generated endpoint builders and wire types. The
//! [`TrustifyClient`] wrapper applies authentication and other shared request
//! behavior to every generated operation.

pub mod api;
pub mod auth;
mod client;
mod pagination;
mod retry;

pub use auth::{AccessTokenProvider, StaticBearerToken};
pub use client::{BuildError, TrustifyClient, TrustifyClientBuilder};
pub use pagination::{OffsetPage, collect_offset_pages};
pub use retry::RetryPolicy;
