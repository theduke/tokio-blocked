//! This crate provides [`TokioBlockedLayer`], a tracing-rs layer that
//! tracks when tokio tasks are blocked by synchronous code.
//!
//! See [`TokioBlockedLayer`] for more details.
mod layer;

pub use self::layer::TokioBlockedLayer;
