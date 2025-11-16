FROM rust:1.88-slim

RUN apt-get update && apt-get install -y tmux bsdmainutils && apt-get clean

COPY . .
RUN cargo build --release && cargo build --package e2e --tests

ENTRYPOINT ["sh"]
CMD ["-c", "tmux new-session -d && cargo e2e -j8"]