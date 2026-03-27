ARG BUILD_PROFILE=release

FROM --platform=$BUILDPLATFORM rust:1.93.0-slim-bookworm@sha256:776861219cd851131c1cec3bbd7cbeb16b99a794048097eb69ad9682a8ed0d57 AS chef
WORKDIR /app
RUN apt-get update && apt-get install --no-install-recommends -y git && rm -rf /var/lib/apt/lists/* && cargo install cargo-chef

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
ENV RUSTFLAGS="-D warnings"
RUN if [ "$BUILD_PROFILE" = "release" ]; then \
      cargo build --release --bin stickerbomb && \
      cp target/release/stickerbomb /app/stickerbomb; \
    else \
      cargo build --bin stickerbomb && \
      cp target/debug/stickerbomb /app/stickerbomb; \
    fi

FROM gcr.io/distroless/cc-debian12:nonroot@sha256:dc65e8ce812dac0f34ca456729ba0cb8a7c1b7c71078be099fb12390a33c4c31
COPY --from=builder /app/stickerbomb /usr/local/bin/stickerbomb
EXPOSE 8080
USER 65532:65532
ENTRYPOINT ["/usr/local/bin/stickerbomb"]
