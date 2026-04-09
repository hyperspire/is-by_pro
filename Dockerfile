# syntax=docker/dockerfile:1

FROM rust:1.86-bookworm AS builder
WORKDIR /app

RUN apt-get update \
  && apt-get install -y --no-install-recommends git ca-certificates \
  && rm -rf /var/lib/apt/lists/*

RUN git clone https://github.com/hyperspire/is-by_pro.git
WORKDIR /app/is-by_pro

RUN cargo build --release

FROM oraclelinux-10-slim AS runtime
WORKDIR /app

RUN yum update -y \
    && yum install -y ca-certificates curl \
    && yum clean all

COPY --from=builder /app/is-by_pro/target/release/is-by_pro /usr/local/bin/is-by_pro
COPY --from=builder /app/is-by_pro/webroot ./webroot
COPY ssl /usr/local/bin/ssl
COPY .env /usr/local/bin/.env
COPY healthcheck.sh /usr/local/bin/healthcheck.sh

RUN chmod +x /usr/local/bin/healthcheck.sh

EXPOSE 80 443

HEALTHCHECK --interval=30s --timeout=10s --start-period=20s --retries=3 \
  CMD ["/usr/local/bin/healthcheck.sh"]

CMD ["/usr/local/bin/is-by_pro"]
