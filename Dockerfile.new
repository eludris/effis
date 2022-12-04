# syntax=docker/dockerfile:1
FROM rust:slim-buster as builder

WORKDIR /effis

COPY Cargo.lock Cargo.toml ./
COPY ./src ./src
COPY ./migrations ./migrations

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/effis/target \
    cargo build --release
# Other image cannot access the target folder.
RUN --mount=type=cache,target=/effis/target \
    cp ./target/release/effis /usr/local/bin/effis

FROM debian:buster-slim

COPY --from=builder /usr/local/bin/effis /bin/effis

COPY migrations ./migrations

# Don't forget to also publish these ports in the docker-compose.yml file.
ARG PORT=7161

EXPOSE $PORT
ENV ROCKET_ADDRESS 0.0.0.0
ENV ROCKET_PORT $PORT

ENV RUST_LOG debug

CMD ["/bin/effis"]
