ARG BUILD_PROFILE=release

FROM --platform=$BUILDPLATFORM rust:1.93.0-slim-bookworm@sha256:38d9e7c33a262bf1c58aecfbdf778205491d703a2196d4abf459e81cfe9f95e4 AS chef
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

FROM gcr.io/distroless/cc-debian12:nonroot@sha256:7e5b8df2f4d36f5599ef4ab856d7d444922531709becb03f3368c6d797d0a5eb
COPY --from=builder /app/stickerbomb /usr/local/bin/stickerbomb
EXPOSE 8080
USER 65532:65532
ENTRYPOINT ["/usr/local/bin/stickerbomb"]
