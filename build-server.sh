#!/bin/sh
exec cargo build --no-default-features --features server "$@"
