FROM rust:1.76.0-slim-buster as build

# create a new empty shell project
RUN USER=root cargo new --bin rinha-servico-rust
WORKDIR /rinha-servico-rust

# copy over your manifests
COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml

# this build step will cache your dependencies
RUN cargo build --release
RUN rm src/*.rs

# copy your source tree
COPY ./src ./src

# build for release
RUN rm ./target/release/deps/rinha_servico_rust*
RUN cargo build --release

# our final base
FROM rust:1.76.0-slim-buster

# copy the build artifact from the build stage
COPY --from=build /rinha-servico-rust/target/release/rinha-servico-rust .

# Expose port 8080 to the outside world
EXPOSE 8080

# Run the binary
CMD ["./rinha-servico-rust", "8080"]
