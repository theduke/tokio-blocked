// use std::time::Duration;
//
// use tokio_blocked::TokioBlockedLayer;
// use tracing_subscriber::{
//     layer::SubscriberExt as _, util::SubscriberInitExt as _, EnvFilter, Layer,
// };
//
// #[test]
// fn main() {
//     tracing_subscriber::registry()
//         .with(tracing_subscriber::fmt::layer().with_filter(EnvFilter::from_default_env()))
//         .with(TokioBlockedLayer::new())
//         .init();
//
//     tracing::info!("Tokio Blocked Layer initialized");
//
//     let rt = tokio::runtime::Builder::new_multi_thread()
//         .enable_all()
//         .build()
//         .expect("Failed to create Tokio runtime");
//
//     rt.block_on(async {
//         tokio::task::spawn(async {
//             eprintln!("task start");
//             std::thread::sleep(Duration::from_secs(1));
//             eprintln!("task end");
//         })
//         .await
//     })
//     .unwrap();
// }
