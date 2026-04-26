FROM rust:slim-trixie

WORKDIR /workspace

RUN apt-get update && apt-get install -y \
    git \
    curl \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

RUN rustup component add rustfmt clippy

CMD [ "bash" ]