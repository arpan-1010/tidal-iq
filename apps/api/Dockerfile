FROM rust:1-bookworm AS build
WORKDIR /app
RUN apt-get update && apt-get install -y pkg-config libssl-dev
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates libssl3 \
    && rm -rf /var/lib/apt/lists/*
COPY --from=build /app/target/release/tidaliq-api /usr/local/bin/tidaliq-api
ENV PORT=4000
EXPOSE 4000
CMD ["tidaliq-api"]