# This is the build stage for X3 Chain. Here we create the binary.
FROM docker.io/library/rust:1.85-slim as builder

WORKDIR /x3-chain
COPY . /x3-chain

# Install required dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    clang \
    && rm -rf /var/lib/apt/lists/*

# Install wasm32 target
RUN rustup target add wasm32-unknown-unknown

# Build the X3 Chain node
RUN cargo build --release --features default

# This is the 2nd stage: a very small image where we copy the X3 Chain binary.
FROM docker.io/library/ubuntu:20.04
LABEL description="Multistage Docker image for X3 Chain: EVM-SVM interoperability blockchain" \
	io.parity.image.type="builder" \
	io.parity.image.authors="x3-chain-dev" \
	io.parity.image.vendor="X3 Chain" \
	io.parity.image.description="X3 Chain is a Substrate-based Layer-1 blockchain enabling native interoperability between Ethereum Virtual Machine (EVM) and Solana Virtual Machine (SVM) execution" \
	io.parity.image.source="https://github.com/x3-chain/x3-chain" \
	io.parity.image.documentation="https://github.com/x3-chain/x3-chain/"

COPY --from=builder /x3-chain/target/release/x3-chain-node /usr/local/bin

RUN useradd -m -u 1000 -U -s /bin/sh -d /x3-chain x3 && \
	mkdir -p /data /x3-chain/.local/share/x3-chain && \
	chown -R x3:x3 /data && \
	ln -s /data /x3-chain/.local/share/x3-chain && \
# Sanity checks
	ldd /usr/local/bin/x3-chain-node && \
# unclutter and minimize the attack surface
	rm -rf /usr/bin /usr/sbin && \
	/usr/local/bin/x3-chain-node --version

USER x3
EXPOSE 30333 9933 9944 9615
VOLUME ["/data"]