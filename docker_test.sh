#!/usr/bin/env bash

# Build a simple Docker image with all development dependencies
# Eventually, run the integration tests.

# Use volumes to cache all the build artifacts
docker volume create registry
docker volume create git
docker volume create build_target
docker volume create elusiv_target
docker volume create elusiv_warden_network_target

docker build -t elusiv-dev:latest .

docker run -it --rm \
  --mount type=volume,source=registry,target=/usr/local/cargo/registry \
  --mount type=volume,source=git,target=/usr/local/cargo/git \
  --mount type=volume,source=build_target,target=/workdir/build/target \
  --mount type=volume,source=elusiv_target,target=/workdir/elusiv/target \
  --mount type=volume,source=elusiv_warden_network_target,target=/workdir/elusiv-warden-network/target \
  -v ./:/workdir \
  -e RUST_BACKTRACE=1 \
  elusiv-dev sh -c '
  sh ./build.sh test --test-kind unit --target elusiv \
  && sh ./build.sh test --test-kind integration --target elusiv \
  && sh ./build.sh test --test-kind unit --target elusiv-warden-network \
  && sh ./build.sh test --test-kind integration --target elusiv-warden-network'