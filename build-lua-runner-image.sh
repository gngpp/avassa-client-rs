#!/bin/bash

docker run --rm -i \
    -v /var/run/docker.sock:/var/run/docker.sock \
    -v $(pwd):/src rust:1-alpine sh <<EOF
apk add libc-dev openssl-dev docker-cli
export CARGO_TARGET_DIR=alpine-target
cd src
cargo build --release
docker build -t fredrikjanssonse/runner -f lua-runner/Dockerfile .
EOF
