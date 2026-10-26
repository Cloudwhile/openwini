# 发布镜像：前端和 Rust 分别编译，由 Rust 统一提供页面及 API。
FROM node:24.22.1-alpine3.24@sha256:06d7adf3e0ef61a13f1d28682726366f97a15aa1888c934d3ecec86c33918b53 AS web
WORKDIR /build
COPY package.json package-lock.json ./
COPY apps/web/package.json apps/web/package.json
COPY packages/ui/package.json packages/ui/package.json
RUN --mount=type=cache,target=/root/.npm npm ci --no-audit --no-fund
COPY apps/web apps/web
COPY packages/ui packages/ui
RUN npm run build --workspace apps/web

FROM rust:1.99.1-bookworm@sha256:48b7f435b8fc6378ce43b44f15e3a47fd8919b2825f835c39bd4bd3da50af5b8 AS builder
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY crates/api/Cargo.toml crates/api/Cargo.toml
# 依赖预取不编译占位程序，下载失败应直接中止构建。
RUN mkdir -p crates/api/src && printf 'fn main() {}\n' > crates/api/src/main.rs && cargo fetch --locked
COPY crates/api/src crates/api/src
COPY migrations migrations
RUN cargo build --release --locked -p wini-api

FROM debian:bookworm-slim@sha256:7ff6ba31527b9e336162fa061746c366c4b2032f4d2cff2078bea43f78efe3f8
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid 10001 wini && useradd --uid 10001 --gid wini --no-create-home wini
WORKDIR /app
COPY --from=builder /build/target/release/wini-api /usr/local/bin/wini-api
COPY --from=web /build/apps/web/dist /web
ENV APP_ENV=production \
    BIND_ADDR=0.0.0.0:10180 \
    WEB_DIST_DIR=/web \
    COOKIE_SECURE=true
USER 10001:10001
EXPOSE 10180
HEALTHCHECK --interval=30s --timeout=5s --start-period=180s --retries=3 \
    CMD curl --fail --silent --max-time 4 http://127.0.0.1:10180/readyz || exit 1
ENTRYPOINT ["wini-api"]
