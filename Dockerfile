FROM ubuntu:24.04

# Build tools
RUN apt-get update && apt-get install -y \
    wget curl git \
    build-essential

# Install rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

COPY ./ /atlas
