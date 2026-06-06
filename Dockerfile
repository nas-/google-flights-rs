# Multi-stage build for the `gflights` CLI.
#
# TLS is rustls with bundled webpki roots, and protoc is vendored by build.rs,
# so the build needs no system OpenSSL, no system protoc, and the runtime image
# needs no ca-certificates package.

FROM rust:1-bookworm AS builder
WORKDIR /app
COPY . .
RUN cargo build --release --bin gflights

FROM debian:bookworm-slim
RUN useradd --create-home --uid 10001 app
COPY --from=builder /app/target/release/gflights /usr/local/bin/gflights
USER app
ENTRYPOINT ["gflights"]
