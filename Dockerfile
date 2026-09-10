# Multi-stage build for linux-x11-sandbox.
# The resulting image contains the MCP server binary and a minimal X11 runtime.

FROM rust:1-bookworm AS builder

RUN apt-get update && apt-get install -y \
    xvfb \
    openbox \
    libatspi2.0-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    xvfb \
    openbox \
    libatspi2.0-0 \
    at-spi2-core \
    dbus-daemon \
    libgtk-3-0 \
    libgdk-pixbuf2.0-0 \
    libx11-6 \
    libxcomposite1 \
    libxdamage1 \
    libxfixes3 \
    libxrandr2 \
    libxkbcommon0 \
    libgbm1 \
    libasound2 \
    fonts-noto-cjk \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/linux-x11-sandbox /usr/local/bin/linux-x11-sandbox
RUN chmod +x /usr/local/bin/linux-x11-sandbox

ENV DISPLAY=:99

ENTRYPOINT ["linux-x11-sandbox"]
