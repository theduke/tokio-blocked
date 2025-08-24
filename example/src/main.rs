use std::time::Duration;
use tokio_blocked::TokioBlockedLayer;
use tracing_subscriber::{
    EnvFilter, Layer as _, layer::SubscriberExt as _, util::SubscriberInitExt as _,
};

#[tokio::main]
async fn main() {
    // Prepare the tracing-subscriber logger with both a regular format logger
    // and the TokioBlockedLayer.

    {
        let fmt = tracing_subscriber::fmt::layer().with_filter(EnvFilter::from_default_env());

        let blocked =
            TokioBlockedLayer::new().with_warn_busy_single_poll(Some(Duration::from_millis(1)));

        tracing_subscriber::registry()
            .with(fmt)
            .with(blocked)
            .init();
    }

    tokio::task::spawn(async {
        // BAD!
        std::thread::sleep(Duration::from_secs(1));
    })
    .await
    .unwrap();

    tokio::task::spawn(async {
        for _ in 0..10 {
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    })
    .await
    .unwrap();
}
