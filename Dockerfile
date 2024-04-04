FROM rust:latest

WORKDIR /kellnr

COPY . .

RUN cargo build --release

CMD ["./target/release/kellnr"]