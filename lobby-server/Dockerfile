# Dockerfile for creating a statically-linked Rust application using docker's
# multi-stage build feature. This also leverages the docker build cache to avoid
# re-downloading dependencies if they have not changed.
FROM rust:1.45.0 AS build
EXPOSE 3450
WORKDIR /usr/src

# Download the target for static linking.
RUN rustup target add x86_64-unknown-linux-musl

# Create a dummy project and build the app's dependencies.
RUN USER=root cargo new lobby-server
RUN USER=root cargo new --lib lobby-server/lobby
WORKDIR /usr/src/lobby-server/lobby
COPY lobby/Cargo.toml ./
RUN cargo build --release

WORKDIR /usr/src/lobby-server
COPY Cargo.toml ./
RUN cargo build --release

# Copy the source and build the application.
COPY src ./src
COPY lobby ./lobby
RUN cargo install --target x86_64-unknown-linux-musl --path .

# Copy the statically-linked binary into a scratch container.
FROM scratch
COPY --from=build /usr/local/cargo/bin/lobby-server .
USER 1000
CMD ["./lobby-server"]
