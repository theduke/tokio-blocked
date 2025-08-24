#!/usr/bin/env sh

RUSTFLAGS="--cfg=tokio_unstable" RUST_LOG=warn cargo run
