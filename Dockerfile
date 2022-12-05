FROM rust:slim-buster as builder

RUN USER=root cargo new --bin effis
WORKDIR /effis

COPY Cargo.lock Cargo.toml ./

RUN cargo build --release
RUN rm src/*.rs

COPY ./src ./src

RUN rm ./target/release/deps/effis*

COPY migrations ./migrations

RUN cargo build --release


FROM debian:buster-slim

RUN apt-get update && apt-get install -y ffmpeg

COPY --from=builder /effis/target/release/effis /bin/effis

COPY migrations ./migrations

# Don't forget to also publish these ports in the docker-compose.yml file.
ARG PORT=7161

EXPOSE $PORT
ENV ROCKET_ADDRESS 0.0.0.0
ENV EFFIS_PORT $PORT

ENV RUST_LOG debug

CMD ["/bin/effis"]

