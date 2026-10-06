# syntax=docker/dockerfile:1

# ---- build the server (native) ----
FROM rust:1-alpine AS server-builder
RUN apk add --no-cache musl-dev
WORKDIR /build
COPY Cargo.toml ./
COPY crates crates
# build only the server: the web crate needs the wasm target
RUN cargo build --release --bin tod-server

# ---- build the web console (wasm + trunk) ----
FROM rust:1-alpine AS web-builder
RUN apk add --no-cache musl-dev
RUN rustup target add wasm32-unknown-unknown
RUN cargo install trunk --version 0.21
WORKDIR /build
COPY Cargo.toml ./
COPY crates/web crates/web
RUN cd crates/web && trunk build --release

# ---- runtime ----
FROM alpine:3.20
RUN apk add --no-cache git ca-certificates
COPY --from=server-builder /build/target/release/tod-server /usr/local/bin/tod-server
COPY --from=web-builder /build/crates/web/dist /opt/tod/web
VOLUME /data
EXPOSE 8080
WORKDIR /data
ENTRYPOINT ["tod-server"]
CMD ["--port", "8080", "--db", "/data/tod-server.db", "--workdir", "/data/repos", "--static", "/opt/tod/web"]
