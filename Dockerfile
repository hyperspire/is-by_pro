# syntax=docker/dockerfile:1

ARG RUST_VERSION=1.94.1
FROM rust:${RUST_VERSION}-bookworm AS builder
WORKDIR /is-by_pro/

FROM fedora:latest AS runtime
WORKDIR $HOME/is-by_pro/

RUN dnf update -y\
  && dnf install -y ca-certificates curl gcc gcc-c++ \
  && dnf clean all

COPY . .

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"
RUN source /root/.cargo/env && rustup default ${RUST_VERSION}
RUN cargo build --release

COPY --from=builder /is-by_pro/target/release/is-by_pro /usr/local/bin/is-by_pro
COPY --from=builder /is-by_pro/webroot ./webroot
COPY --from=builder /is-by_pro/ssl /usr/local/bin/ssl
COPY --from=builder /is-by_pro/.env /usr/local/bin/.env
COPY healthcheck.sh /usr/local/bin/healthcheck.sh

RUN chmod +x /usr/local/bin/healthcheck.sh

EXPOSE 80 443

HEALTHCHECK --interval=30s --timeout=10s --start-period=20s --retries=3 \
  CMD ["/usr/local/bin/healthcheck.sh"]

CMD ["/usr/local/bin/is-by_pro"]
