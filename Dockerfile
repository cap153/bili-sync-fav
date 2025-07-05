# ==================================
# STAGE 1: The Builder
# ==================================
FROM rust:1.85 AS builder

# 1. 准备 C 库环境 (FFmpeg 编译)
RUN \
    sed -i 's/ main/ main contrib non-free non-free-firmware/g' /etc/apt/sources.list.d/debian.sources && \
    apt-get update && \
    apt-get install -y --no-install-recommends \
        build-essential cmake git nasm yasm libx264-dev libx265-dev libvpx-dev libfdk-aac-dev \
        libmp3lame-dev libopus-dev libssl-dev libsqlite3-dev pkg-config libclang-dev && \
    mkdir -p /usr/src/ffmpeg_sources && cd /usr/src/ffmpeg_sources && \
    FFMPEG_VERSION=7.0 && git clone --depth 1 --branch n${FFMPEG_VERSION} https://git.ffmpeg.org/ffmpeg.git ffmpeg && \
    cd ffmpeg && \
    ./configure --prefix=/usr/local --enable-gpl --enable-version3 --enable-nonfree --enable-shared \
        --enable-libx264 --enable-libx265 --enable-libvpx --enable-libfdk-aac --enable-libmp3lame --enable-libopus && \
    make -j$(nproc) && make install && \
    cd / && rm -rf /usr/src/ffmpeg_sources

# 2. 准备 Rust 环境并编译项目
WORKDIR /usr/src/app
ENV PKG_CONFIG_PATH="/usr/local/lib/pkgconfig"

# 先复制所有文件
COPY . .
# 然后直接构建
RUN cargo build --release --locked

# ==================================
# STAGE 2: The Runtime 
# ==================================
FROM debian:bookworm-slim

# 安装运行时的必要共享库
RUN \
    sed -i 's/ main/ main contrib non-free non-free-firmware/g' /etc/apt/sources.list.d/debian.sources && \
    apt-get update && \
    apt-get install -y --no-install-recommends \
        ca-certificates libsqlite3-0 libx264-164 libx265-199 libvpx7 \
        libfdk-aac2 libmp3lame0 libopus0 libssl3 \
    && rm -rf /var/lib/apt/lists/*

# 复制 builder 中 /usr/local/lib 里的内容，这是我们自己编译的
COPY --from=builder /usr/local/lib/libav*.so.* /usr/local/lib/
COPY --from=builder /usr/local/lib/libsw*.so.* /usr/local/lib/
RUN echo "/usr/local/lib" > /etc/ld.so.conf.d/ffmpeg.conf && ldconfig

# 从构建阶段复制编译好的二进制文件
COPY --from=builder /usr/src/app/target/release/bili-sync-fav /usr/local/bin/

# 设置工作目录
WORKDIR /app

# 指定日志等级
ENV RUST_LOG="bili_sync_fav=info,warn"

# ENTRYPOINT 只负责指定要运行的程序
ENTRYPOINT ["bili-sync-fav"]

# CMD 提供默认参数
CMD ["-c", "config.toml"]
