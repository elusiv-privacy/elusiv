FROM rust:1.65-bullseye
WORKDIR /workdir
RUN apt-get update && apt-get install -y pkg-config build-essential libudev-dev libelf-dev libclang-dev
RUN sh -c "$(curl -sSfL https://release.solana.com/v1.10.39/install)"
ENV PATH="/root/.local/share/solana/install/active_release/bin:$PATH"
# Use a temporary crate to install cargo-test-bpf
RUN cargo new temp && cd temp \
  && cargo-test-bpf && cd .. && rm -R temp