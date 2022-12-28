FROM rust:1.66-alpine AS builder

RUN apk add --no-cache musl-dev

WORKDIR /build

COPY . .

RUN cargo build --release

FROM alpine

COPY --from=builder /build/target/release/mmproxy /usr/bin/

ENTRYPOINT [ "mmproxy" ]
