#!/bin/sh
set -eu
cargo build --locked --bin plrc
cargo test --all-targets --locked
