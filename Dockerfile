ARG BUILD_PROFILE=release

FROM --platform=$BUILDPLATFORM rust:1.92.0-bookworm as chef
WORKDIR /app
RUN cargo install cargo-chef

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
RUN if [ "$BUILD_PROFILE" = "release" ]; then \
      cargo build --release --bin stickerbomb && \
      cp target/release/stickerbomb /app/stickerbomb; \
    else \
      cargo build --bin stickerbomb && \
      cp target/debug/stickerbomb /app/stickerbomb; \
    fi

FROM gcr.io/distroless/cc-debian12:nonroot
COPY --from=builder /app/stickerbomb /usr/local/bin/stickerbomb
EXPOSE 8080
USER 65532:65532
ENTRYPOINT ["/usr/local/bin/stickerbomb"]
