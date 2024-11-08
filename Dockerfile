# Usage:
# docker build -f Dockerfile -q . | xargs -I % docker run %
# Allows to easily run locally python tests in isolated container for reproducibility.

# Use Ubuntu as the base image
FROM ubuntu:latest

# Set environment variables
ENV LC_ALL=en_US.UTF-8
ENV TERM=xterm-256color
ENV RUST_VERSION=stable

# Install dependencies
RUN apt-get update && apt-get install -y \
    curl \
    build-essential \
    zsh \
    python3 \
    tmux \
    locales \
    && rm -rf /var/lib/apt/lists/*

# Set up locale
RUN locale-gen en_US.UTF-8

# Install Rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain $RUST_VERSION
ENV PATH="/root/.cargo/bin:${PATH}"

# Set working directory
WORKDIR /app

# Copy your project files
COPY . .

# Build the project
RUN cargo build --release

# Run tests
CMD tmux new-session -d && python3 test/test_skim.py --verbose

# Additional commands for other checks (uncomment if needed)
# CMD cargo clippy
# CMD cargo fmt --all -- --check
