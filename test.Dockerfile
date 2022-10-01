# Usage:
# docker build -f Dockerfile -q . | xargs -I % docker run %
# Allows to easily run locally python tests in isolated container for reproducibility.

# Use Ubuntu as the base image
FROM rust:1.82-slim

# Set environment variables
ENV LC_ALL=en_US.UTF-8
ENV TERM=xterm-256color

# Install dependencies
RUN apt-get update && apt-get install -y \
    curl \
    build-essential \
    zsh \
    python3 \
    tmux \
    locales \
    xxd \
    && rm -rf /var/lib/apt/lists/* \
    && apt-get -y clean

# Set up locale
RUN locale-gen en_US.UTF-8

# Set working directory
WORKDIR /app

# Copy your project files
COPY . .

# Build the project
RUN cargo build --release

# Run tests
CMD tmux new-session -d && python3 test/test_skim.py --verbose
