FROM nvidia/cuda:12.4.0-devel-ubuntu22.04 AS builder

ARG DEBIAN_FRONTEND=noninteractive

ARG CUDA_ARCH=89
ENV CUDAARCHS=${CUDA_ARCH}

# set of args for use of sccache
ARG SCCACHE_GCS_BUCKET=""
ARG SCCACHE_GCS_SERVICE_ACCOUNT=""
ARG SCCACHE_GCS_RW_MODE=""
ARG RUSTC_WRAPPER=""

ENV SCCACHE_GCS_BUCKET=${SCCACHE_GCS_BUCKET}
ENV SCCACHE_GCS_SERVICE_ACCOUNT=${SCCACHE_GCS_SERVICE_ACCOUNT}
ENV SCCACHE_GCS_RW_MODE=${SCCACHE_GCS_RW_MODE}
ENV RUSTC_WRAPPER=${RUSTC_WRAPPER}

RUN apt-get update && apt-get install -y curl clang openssl libssl-dev gcc g++ \
    pkg-config build-essential libclang-dev && \
    rm -rf /var/lib/apt/lists/*

ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH

RUN curl https://sh.rustup.rs -sSf | bash -s -- -y && \
    rustup install nightly-2024-09-01 && \
    rustup default nightly-2024-09-01

RUN curl -Lo cmake-3.24.2-linux-x86_64.sh https://github.com/Kitware/CMake/releases/download/v3.24.2/cmake-3.24.2-linux-x86_64.sh && \
    chmod +x cmake-3.24.2-linux-x86_64.sh && \
    ./cmake-3.24.2-linux-x86_64.sh --skip-license --prefix=/usr/local

# install sccache
RUN curl -Lo sccache-v0.8.1-x86_64-unknown-linux-musl.tar.gz https://github.com/mozilla/sccache/releases/download/v0.8.1/sccache-v0.8.1-x86_64-unknown-linux-musl.tar.gz && \
    tar -xzf sccache-v0.8.1-x86_64-unknown-linux-musl.tar.gz && \
    cp sccache-v0.8.1-x86_64-unknown-linux-musl/sccache /usr/local/sbin/ && \
    rm -rf sccache-v0.8.1-x86_64-unknown-linux-musl.tar.gz && \
    rm -rf sccache-v0.8.1-x86_64-unknown-linux-musl && \
    chmod +x /usr/local/sbin/sccache

WORKDIR /usr/src/zksync
COPY . .

RUN cd prover && cargo build --release --package zksync_circuit_prover

# Output build in CUDA architectures for debugging purposes.
RUN cuobjdump /usr/src/zksync/prover/target/release/zksync_circuit_prover

FROM nvidia/cuda:12.4.0-runtime-ubuntu22.04

RUN apt-get update && apt-get install -y curl libpq5 ca-certificates && rm -rf /var/lib/apt/lists/*

# copy finalization hints required for assembly generation
COPY prover/data/keys/ /prover/data/keys/

COPY --from=builder /usr/src/zksync/prover/target/release/zksync_circuit_prover /usr/bin/

ENTRYPOINT ["zksync_circuit_prover"]
