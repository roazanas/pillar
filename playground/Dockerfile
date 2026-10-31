FROM rust:1.99.1-bookworm AS builder
WORKDIR /build
COPY Cargo.toml Cargo.lock rust-toolchain.toml build.rs runtime.c ./
COPY src ./src
RUN cargo build --release --locked --bin plrc

FROM python:3.14-slim-bookworm
RUN apt-get update && apt-get install -y --no-install-recommends gcc libc6-dev && rm -rf /var/lib/apt/lists/*
COPY --from=builder /build/target/release/plrc /usr/local/bin/plrc
COPY playground/runner.py /runner.py
ENV PILLAR_SANDBOX=1 PYTHONDONTWRITEBYTECODE=1
WORKDIR /work
USER 65534:65534
ENTRYPOINT ["python3", "-I", "/runner.py"]
