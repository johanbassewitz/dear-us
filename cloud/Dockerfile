# Small image for a ~5 EUR VPS: build once, run a single static-ish binary.
FROM rust:1-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY migrations migrations
COPY src src
RUN cargo build --release --locked

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/* \
 && useradd --system --uid 10001 dearus
COPY --from=build /src/target/release/dear-us-cloud /usr/local/bin/dear-us-cloud
USER dearus
EXPOSE 8080
CMD ["dear-us-cloud"]
