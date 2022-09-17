FROM rust:slim-buster as builder

RUN USER=root cargo new --bin effis
WORKDIR /effis

COPY Cargo.lock Cargo.toml ./

RUN cargo build --release
RUN rm src/*.rs

COPY ./src Rocket.toml ./

RUN rm ./target/release/deps/effis*
RUN cargo build --release


FROM debian:buster-slim

COPY --from=builder /effis/target/release/effis /bin/effis

COPY Rocket.toml Rocket.toml

# Don't forget to also publish these ports in the docker-compose.yml file.
ARG PORT=7161

EXPOSE $PORT
ENV ROCKET_ADDRESS 0.0.0.0
ENV ROCKET_PORT $PORT

ENV RUST_LOG debug

CMD ["/bin/effis"]

