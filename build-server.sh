#!/bin/sh
export RUSTFLAGS="$RUSTFLAGS -A dead_code"
exec cargo build --no-default-features --features server "$@"
