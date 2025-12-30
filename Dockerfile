ARG BUILD_PROFILE=release

FROM --platform=$BUILDPLATFORM rust:1.92.0-slim-bookworm@sha256:376e6785918280aa68bef2d8d7b0204b58dfd486f370419023363c6e8cc09ec3 as chef
WORKDIR /app
RUN apt-get update && apt-get install -y git && rm -rf /var/lib/apt/lists/* && cargo install cargo-chef

FROM chef AS planner
COPY Cargo.toml Cargo.lock ./
COPY crates crates
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
ARG BUILD_PROFILE
ARG TARGETPLATFORM
COPY --from=planner /app/recipe.json recipe.json
RUN if [ "$BUILD_PROFILE" = "release" ]; then \
      cargo chef cook --release --recipe-path recipe.json; \
    else \
      cargo chef cook --recipe-path recipe.json; \
    fi

COPY . .
ENV RUSTFLAGS -D warnings
RUN if [ "$BUILD_PROFILE" = "release" ]; then \
      cargo build --release --bin stickerbomb && \
      cp target/release/stickerbomb /app/stickerbomb; \
    else \
      cargo build --bin stickerbomb && \
      cp target/debug/stickerbomb /app/stickerbomb; \
    fi

FROM gcr.io/distroless/cc-debian12:nonroot@sha256:2575808fe33e2a728348040ef2fd6757b0200a73ca8daebd0c258e2601e76c6d
COPY --from=builder /app/stickerbomb /usr/local/bin/stickerbomb
EXPOSE 8080
USER 65532:65532
ENTRYPOINT ["/usr/local/bin/stickerbomb"]
