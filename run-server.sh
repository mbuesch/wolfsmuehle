#!/bin/sh
exec cargo run --no-default-features --features server "$@"
