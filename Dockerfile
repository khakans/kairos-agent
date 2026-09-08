FROM node:24-bookworm-slim AS web
WORKDIR /build
COPY package.json package-lock.json ./
RUN npm ci
COPY src ./src
COPY public ./public
COPY index.html tsconfig.json tsconfig.node.json vite.config.ts ./
RUN npm run build

FROM rust:1.98-slim-bookworm AS rust
RUN apt-get update && apt-get install -y --no-install-recommends build-essential pkg-config ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY apps ./apps
COPY src-tauri ./src-tauri
RUN cargo build --locked --release -p kairos-server

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/* && useradd --uid 10001 --create-home kairos && mkdir /data && chown kairos:kairos /data
WORKDIR /app
COPY --from=rust /build/target/release/kairos-server /app/kairos-server
COPY --from=web /build/dist /app/dist
ENV KAIROS_BIND=0.0.0.0:8080 KAIROS_DATA_DIR=/data KAIROS_WEB_DIR=/app/dist
USER kairos
VOLUME ["/data"]
EXPOSE 8080
CMD ["/app/kairos-server"]
