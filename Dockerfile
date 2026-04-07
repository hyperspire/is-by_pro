# syntax=docker/dockerfile:1

FROM rust:1.86-bookworm AS builder
WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN cargo build --release

FROM oraclelinux-10-slim AS runtime
WORKDIR /app

RUN yum update -y \
    && yum install -y ca-certificates curl \
    && yum clean all

COPY --from=builder /app/target/release/is-by_pro /usr/local/bin/is-by_pro
COPY webroot ./webroot
COPY ssl ./ssl
COPY .env ./env
COPY healthcheck.sh /usr/local/bin/healthcheck.sh

RUN chmod +x /usr/local/bin/healthcheck.sh

EXPOSE 80 443

HEALTHCHECK --interval=30s --timeout=10s --start-period=20s --retries=3 \
  CMD ["/usr/local/bin/healthcheck.sh"]

CMD ["/usr/local/bin/is-by_pro"]
