FROM rust:1.93-alpine

EXPOSE 7447

RUN apk add --no-cache musl-dev

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN cargo build --release

CMD ["./target/release/send-it"]
