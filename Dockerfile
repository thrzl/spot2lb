FROM debian:bookworm-slim AS prep-runtime
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        libssl3 \
        ca-certificates && \
    rm -rf /var/lib/apt/lists/*


FROM lukemathwalker/cargo-chef AS chef
RUN cargo install cargo-chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release --bin spotbrainz

FROM prep-runtime AS runtime
WORKDIR /app
COPY --from=builder /app/target/release/spotbrainz /usr/local/bin
ENTRYPOINT ["/usr/local/bin/spotbrainz"]
