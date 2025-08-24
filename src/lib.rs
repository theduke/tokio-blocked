//! This crate provides [`TokioBlockedLayer`], a tracing-rs layer that
//! tracks when tokio tasks are blocked by synchronous code.
//!
//! See [`TokioBlockedLayer`] for more details.
//!
//! ## Usage
//!
//! Add this to your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! tracing = "0.1"
//! tracing-subscriber = { version = "0.3", features = ["fmt", "env-filter"] }
//! tokio = { version = "1.47", features = ["rt-multi-thread", "tracing", "macros", "time"] }
//!
//! tokio-blocked = "*"
//! ````
//!
//! Then, in your application code:
//!
//! ```rust
//! use std::time::Duration;
//! use tokio_blocked::TokioBlockedLayer;
//! use tracing_subscriber::{
//!     EnvFilter, Layer as _, layer::SubscriberExt as _, util::SubscriberInitExt as _,
//! };
//!
//! #[tokio::main]
//! async fn main() {
//!     // Prepare the tracing-subscriber logger with both a regular format logger
//!     // and the TokioBlockedLayer.
//!
//!     {
//!         let fmt = tracing_subscriber::fmt::layer().with_filter(EnvFilter::from_default_env());
//!
//!         let blocked = TokioBlockedLayer::new()
//!             .with_warn_busy_single_poll(Some(Duration::from_micros(150)));
//!
//!         tracing_subscriber::registry()
//!             .with(fmt)
//!             .with(blocked)
//!             .init();
//!     }
//!
//!     tokio::task::spawn(async {
//!         // BAD!
//!         // This produces a warning log message.
//!         std::thread::sleep(Duration::from_secs(2));
//!     })
//!     .await
//!     .unwrap();
//! }
//! ```

mod layer;

pub use self::layer::TokioBlockedLayer;
