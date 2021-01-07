#!/bin/sh
export RUSTFLAGS="$RUSTFLAGS -A dead_code"
exec cargo run --no-default-features --features server "$@"
