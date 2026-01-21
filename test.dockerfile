FROM rust:1-slim

RUN apt-get update && apt-get install -y tmux bsdmainutils && apt-get clean
COPY rust-toolchain.toml .

RUN cargo install cargo-nextest

COPY . .

CMD ["cargo", "nextest", "run", "--release", "--features", "test-utils"]
