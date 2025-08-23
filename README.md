# tokio-blocked

[![Crates.io](https://img.shields.io/crates/v/tokio-blocked.svg)](https://crates.io/crates/tokio-blocked)
[![Docs.rs](https://docs.rs/tokio-blocked/badge.svg)](https://docs.rs/tokio-blocked)
[![License: MIT/Apache-2.0](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](#license)

`tokio-blocked` integrates with Tokios `tracing` integration to detect tasks
that are blocked by synchronus or CPU heavy code,
and surfaces information to developers with log messages or data dumps.

## Why?

One of the most common mistakes in async Rust code is running
synchronous blocking operations or CPU-heavy code inside async tasks.

The recommendation is to offload all code that takes more than 10-100 **microseconds**
by using `tokio::task::spawn_blocking`,  `block_in_place`, ....

Not doing this can lead to mysterious latency spikes, stalls,
and degraded performance that only shows up under load / in production.

## Quickstart

**NOTE**: The tracing feature in tokio is experimental (as of Tokio 1.47).
To make it work, the env var `RUSTFLAGS="--cfg tokio_unstable"` must be set
when building the code.

To use `tokio-blocked`, follow these steps:

In Cargo.toml:
```toml
[dependencies]

# Enable the tracing feature for Tokio
tokio = { version = "1", features = ["tracing", "rt-multi-thread", "macros"] }

# Depend on tokio-blocked and the tracing crates
tokio-blocked = "*"

tracing = "0.1.41"
tracing-subscriber = { version = "0.3.19", features = ["fmt", "env-filter"] }
```

main.rs:
```rust
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

        let blocked = TokioBlockedLayer::new()
            .with_warn_busy_single_poll(Some(Duration::from_micros(150)));

        tracing_subscriber::registry()
            .with(fmt)
            .with(blocked)
            .init();
    }

    tokio::task::spawn(async {
        // BAD!
        // This produces a warning log message.
        std::thread::sleep(Duration::from_secs(2));
    })
    .await
    .unwrap();
}
```

Now the code can be run with:

```bash
RUSTFLAGS="--cfg tokio_unstable" RUST_LOG=warn cargo run
```


## Configuration

TODO


## Notes and Limitations

TODO

## Develop

## Acknowledgements

Thanks to [tokio-console](https://github.com/tokio-rs/console) for examples
of extracting information from the Tokio trace data.

### License

Licensed under either of:

- Apache License, Version 2.0 (LICENSE-APACHE)
- MIT license (LICENSE-MIT)

at your option.

Unless you explicitly state otherwise,
any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license,
shall be dual licensed as above, without any additional terms or conditions.

