FROM rust:1.80.1-slim-bullseye

WORKDIR /app

COPY . .

# Update package lists and install dependencies
RUN apt-get update && apt-get install -y pkg-config libssl-dev
RUN apt-get update && apt-get install -y git

RUN cargo build --release

EXPOSE 8080

CMD ["cargo run --release server"]
