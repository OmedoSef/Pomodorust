FROM rust:1-bookworm

RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    pkg-config \
    libgtk-3-dev \
    libayatana-appindicator3-dev \
    libxdo-dev \
    libssl-dev \
    libdbus-1-dev \
    libasound2-dev \
    && rm -rf /var/lib/apt/lists/*

RUN rustup component add clippy

WORKDIR /workspace
