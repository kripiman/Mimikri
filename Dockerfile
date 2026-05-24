# =============================================================================
# OsintUltimate - Optimized Dockerfile (Alpine + musl + Static Linking)
# =============================================================================
# This version uses Alpine Linux for the runner to minimize RAM and disk footprint.
# Binary is statically linked with musl to ensure portability.
# =============================================================================

# ── Stage 1: Builder ──────────────────────────────────────────────────────────
FROM rust:1-alpine AS builder

RUN apk add --no-cache \
    pkgconfig \
    openssl-dev \
    sqlite-dev \
    musl-dev \
    libgcc

WORKDIR /build

# Copy manifests
COPY Cargo.toml Cargo.lock ./

# Pre-fetch dependencies with dummy source
RUN mkdir src && echo 'fn main() {}' > src/main.rs \
    && echo '' > src/lib.rs \
    && cargo fetch

# Copy source and build
COPY src ./src
# Build statically linked binary
RUN RUSTFLAGS="-C target-feature=+crt-static" cargo build --release --locked --target x86_64-unknown-linux-musl \
    && strip target/x86_64-unknown-linux-musl/release/mimikri

# ── Stage 2: Runner ────────────────────────────────────────────────────────────
FROM alpine:latest AS runner

LABEL maintainer="mimikri <architect@redteam.lab>" \
      org.opencontainers.image.title="mimikri (Alpine Optimized)" \
      org.opencontainers.image.description="High-performance async red team assessment engine"

# Runtime dependencies in Alpine
RUN apk add --no-cache \
    ca-certificates \
    nmap \
    python3 \
    py3-pip \
    git \
    jq \
    unzip \
    curl \
    wget \
    iputils \
    bind-tools \
    libcap

# Install Go tools (ProjectDiscovery stack)
ARG GOARCH=amd64
ENV GOARCH=${GOARCH}

RUN for tool in nuclei httpx subfinder naabu; do \
      curl -sSfL "https://github.com/projectdiscovery/${tool}/releases/latest/download/${tool}_linux_${GOARCH}.zip" -o "/tmp/${tool}.zip" \
      && unzip -q "/tmp/${tool}.zip" -d /usr/local/bin/ "${tool}" \
      && chmod +x "/usr/local/bin/${tool}" \
      && rm "/tmp/${tool}.zip"; \
    done

# ffuf
RUN curl -sSfL "https://github.com/ffuf/ffuf/releases/latest/download/ffuf_$(curl -sSfL https://api.github.com/repos/ffuf/ffuf/releases/latest | jq -r .tag_name | tr -d v)_linux_amd64.tar.gz" \
    -o /tmp/ffuf.tar.gz \
    && tar -xzf /tmp/ffuf.tar.gz -C /usr/local/bin/ ffuf \
    && chmod +x /usr/local/bin/ffuf \
    && rm /tmp/ffuf.tar.gz

# sqlmap
RUN pip3 install --no-cache-dir sqlmap --break-system-packages

# Copy static binary from builder
COPY --from=builder /build/target/x86_64-unknown-linux-musl/release/mimikri /usr/local/bin/mimikri

# Setup non-root user
RUN addgroup -S mimikri && adduser -S mimikri -G redteam -u 1000 \
    && mkdir -p /workspace /output && chown -R mimikri:mimikri /workspace /output

WORKDIR /workspace
USER mimikri

VOLUME ["/workspace", "/output"]
EXPOSE 4317

ENTRYPOINT ["/usr/local/bin/mimikri"]
CMD ["--help"]
