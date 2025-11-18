FROM ubuntu:24.04 

# Build tools
RUN apt-get update && apt-get install -y \
    wget curl git \
    build-essential \
    cmake clang ninja-build

# Install Spark-DSG dependencies
RUN apt-get install -y libeigen3-dev nlohmann-json3-dev libzmq3-dev libgtest-dev

# Install rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

# Set default compiler to clang
ENV CC=clang CXX=clang++

COPY ./ /atlas



