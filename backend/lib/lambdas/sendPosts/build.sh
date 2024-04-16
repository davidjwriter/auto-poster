#!/bin/bash

export OPENSSL_DIR="/usr/lib/x86_64-linux-gnu"
export OPENSSL_INCLUDE_DIR="/usr/include/openssl" 
cargo build --release --target x86_64-unknown-linux-musl
cd target/x86_64-unknown-linux-musl/release && mkdir -p lambda && cp bootstrap lambda/