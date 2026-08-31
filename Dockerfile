# syntax=docker/dockerfile:1.7
# Multi-stage build for systemprompt-template.
# Stage 1 compiles the Rust workspace against the repo's .sqlx/ offline cache.
# Stage 2 ships a slim Debian runtime with the binaries + services/ YAML tree.

FROM rust:1-bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    libpq-dev \
    libssl-dev \
    pkg-config \
    clang \
    mold \
    build-essential \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /src
COPY . /src

ENV SQLX_OFFLINE=true \
    CC=clang \
    CXX=clang++

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --workspace \
    && mkdir -p /out/bin \
    && cp target/release/systemprompt /out/bin/ \
    && cp target/release/systemprompt-mcp-agent /out/bin/

# hey powers the demo/performance load tests; its upstream S3 binary host is
# dead (403), so build it from source and ship it on PATH — demo/_common.sh's
# install_hey() prefers a system hey.
FROM golang:1-bookworm AS heybuilder
RUN --mount=type=cache,target=/go/pkg/mod \
    --mount=type=cache,target=/root/.cache/go-build \
    go install github.com/rakyll/hey@latest

FROM debian:bookworm-slim AS runtime

LABEL org.opencontainers.image.title="systemprompt" \
      org.opencontainers.image.description="AI governance gateway for Claude, OpenAI, and Gemini — policy, audit, and MCP orchestration" \
      org.opencontainers.image.source="https://github.com/systempromptio/systemprompt-template" \
      org.opencontainers.image.url="https://systemprompt.io" \
      org.opencontainers.image.documentation="https://github.com/systempromptio/systemprompt-template/tree/main/docs" \
      org.opencontainers.image.vendor="systemprompt.io"

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    libpq5 \
    libssl3 \
    postgresql-client \
    lsof \
    jq \
    python3 \
    python3-venv \
    # WeasyPrint's rendering stack. Its layout engine is Python with no Rust
    # binding, so the factsheet MCP server shells out to it; these are the
    # system libraries it loads through cffi. libcairo is deliberately absent —
    # WeasyPrint >= 53 writes PDF directly via pydyf and no longer needs it.
    libpango-1.0-0 \
    libpangoft2-1.0-0 \
    libharfbuzz0b \
    libfontconfig1 \
    && rm -rf /var/lib/apt/lists/*

# A venv rather than --break-system-packages: Debian's Python is the system's,
# and WeasyPrint pulls a substantial dependency tree.
RUN python3 -m venv /app/.venv \
    && /app/.venv/bin/pip install --no-cache-dir weasyprint pymupdf

RUN useradd -m -u 1000 app
WORKDIR /app

RUN mkdir -p /app/bin /app/logs /app/storage /app/web /app/services/profiles/docker

COPY --from=builder /out/bin/ /app/bin/
COPY --from=heybuilder /go/bin/hey /app/bin/hey

COPY services /app/services
COPY storage /app/storage
COPY web /app/web
# The homepage demo showcase scans <system_root>/demo at runtime; without
# this the catalogue renders as an empty section.
COPY demo /app/demo
# MCP manifests live alongside their extension crates; the runtime validator
# globs extensions/mcp/*/manifest.yaml to resolve binary -> manifest.
COPY extensions/mcp /app/extensions/mcp

# The factsheet renderer sidecar; the MCP server resolves it relative to the
# system root.
COPY scripts /app/scripts

COPY docker/entrypoint.sh /app/entrypoint.sh
RUN chmod +x /app/entrypoint.sh /app/bin/* \
    && chown -R app:app /app

USER app
EXPOSE 8080

ENV HOST=0.0.0.0 \
    PORT=8080 \
    RUST_LOG=info \
    PATH="/app/bin:${PATH}" \
    SYSTEMPROMPT_SERVICES_PATH=/app/services \
    SYSTEMPROMPT_MCP_PATH=/app/bin \
    SYSTEMPROMPT_PROFILE=/app/.systemprompt/profiles/docker/profile.yaml \
    FACTSHEET_PYTHON=/app/.venv/bin/python3 \
    WEB_DIR=/app/web

HEALTHCHECK --interval=30s --timeout=10s --start-period=30s --retries=3 \
    CMD curl -f http://localhost:8080/api/v1/health || exit 1

ENTRYPOINT ["/app/entrypoint.sh"]
