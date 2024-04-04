FROM rust:latest

WORKDIR /kellnr

COPY . .

CMD ["cargo run --release"]