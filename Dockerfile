# syntax=docker/dockerfile:1
#
# sheetsolver as one image: the API, serving the built web UI at `/`.
# See docs/ARCHITECTURE.md, "Deployment".
#
#   docker build -t sheetsolver .
#   docker run --rm -p 8080:8080 sheetsolver
#
# Settings are environment variables named after the `api` options
# (`api --help`): SHEETSOLVER_LISTEN, SHEETSOLVER_MAX_RUNNING_JOBS, …

ARG RUST_VERSION=1
ARG DEBIAN_RELEASE=bookworm

# --- Rust: the `api` binary, with its dependencies in their own layer ---

FROM rust:${RUST_VERSION}-slim-${DEBIAN_RELEASE} AS chef
ARG CARGO_CHEF_VERSION=0.1.78
RUN cargo install cargo-chef --locked --version "${CARGO_CHEF_VERSION}"
WORKDIR /build

# The dependency list only: unchanged unless a Cargo.toml or Cargo.lock
# changes, so the next stage's dependency build stays cached.
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /build/recipe.json recipe.json
RUN cargo chef cook --release --locked -p api --recipe-path recipe.json
COPY . .
RUN cargo build --release --locked -p api \
    && cp target/release/api /usr/local/bin/api \
    && /usr/local/bin/api --print-openapi > /openapi.json

# --- Web UI ---
#
# Stage 7 has no buildable `web/` yet, so this placeholder gives an empty
# UI directory and the image serves the API alone. Once `web/` builds,
# delete the placeholder and uncomment the Node stage below; it must leave
# the built UI in /web/dist (index.html and its assets). It gets
# openapi.json from the Rust stage, to generate the typed client.

FROM debian:${DEBIAN_RELEASE}-slim AS web
RUN mkdir -p /web/dist

# FROM node:22-bookworm-slim AS web
# WORKDIR /web
# COPY web/package.json web/package-lock.json ./
# RUN npm ci
# COPY web/ ./
# COPY --from=builder /openapi.json ./openapi.json
# RUN npm run build

# --- Runtime: a small image, a non-root user ---

FROM debian:${DEBIAN_RELEASE}-slim
RUN useradd --uid 10001 --no-create-home --shell /usr/sbin/nologin sheetsolver
COPY --from=builder /usr/local/bin/api /usr/local/bin/api
COPY --from=web /web/dist /app/web
ENV SHEETSOLVER_LISTEN=0.0.0.0:8080 \
    SHEETSOLVER_WEB_DIR=/app/web
EXPOSE 8080
USER sheetsolver
ENTRYPOINT ["/usr/local/bin/api"]
