# syntax=docker/dockerfile:1

ARG RUST_VERSION=1.94.1
FROM rust:${RUST_VERSION}-bookworm AS builder
WORKDIR /app

RUN dnf update -y\
  && dnf install -y --no-install-recommends ca-certificates \
  && dnf clean all

COPY . .

RUN cargo build --release

FROM oraclelinux-10-slim AS runtime
WORKDIR /app

RUN dnf update -y \
    && dnf install -y ca-certificates curl \
    && dnf clean all

COPY --from=builder /app/target/release/is-by_pro /usr/local/bin/is-by_pro
COPY --from=builder /app/webroot ./webroot
COPY ssl /usr/local/bin/ssl
COPY .env /usr/local/bin/.env
COPY healthcheck.sh /usr/local/bin/healthcheck.sh

RUN chmod +x /usr/local/bin/healthcheck.sh

EXPOSE 80 443

HEALTHCHECK --interval=30s --timeout=10s --start-period=20s --retries=3 \
  CMD ["/usr/local/bin/healthcheck.sh"]

CMD ["/usr/local/bin/is-by_pro"]
