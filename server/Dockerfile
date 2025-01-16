FROM rust:bullseye AS builder
WORKDIR /src
COPY server .
RUN cargo build --release

FROM debian:bullseye
COPY --from=builder /src/target/release/distance-db-querier-server /usr/local/bin/

CMD ["distance-db-querier-server"]