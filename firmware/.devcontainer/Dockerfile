# PetFilter ESP32-S3 Firmware Development Container
# Base: Espressif idf-rust (Xtensa Rust toolchain + espflash + ldproxy)
FROM espressif/idf-rust:esp32s3_latest

ARG FLATC_VERSION=25.1.21
ARG DEBIAN_FRONTEND=noninteractive

USER root

RUN apt-get update && apt-get install -y --no-install-recommends \
        cmake \
        curl \
        git \
        jq \
        libssl-dev \
        pkg-config \
        python3 \
        python3-pip \
        unzip \
        wget \
        nodejs \
        npm \
    && rm -rf /var/lib/apt/lists/*

# Install FlatBuffers compiler (asset name: Linux.flatc.binary.clang++-18.zip)
RUN curl -fsSL "https://github.com/google/flatbuffers/releases/download/v${FLATC_VERSION}/Linux.flatc.binary.clang%2B%2B-18.zip" \
        -o /tmp/flatc.zip && \
    unzip -o /tmp/flatc.zip -d /usr/local/bin/ && \
    chmod +x /usr/local/bin/flatc && \
    rm /tmp/flatc.zip && \
    flatc --version

# Ensure Xtensa toolchain env vars are available in non-interactive shells
RUN echo 'source /home/esp/export-esp.sh' >> /etc/bash.bashrc && \
    { echo '#!/bin/sh'; cat /home/esp/export-esp.sh; } > /etc/profile.d/esp-toolchain.sh && \
    chmod +x /etc/profile.d/esp-toolchain.sh

# Allow esp user to fix volume mount ownership at runtime
RUN apt-get update && apt-get install -y --no-install-recommends sudo && rm -rf /var/lib/apt/lists/* && \
    mkdir -p /etc/sudoers.d && \
    echo 'esp ALL=(root) NOPASSWD: /bin/chown -R esp\:esp /workspace/target, /bin/chown -R esp\:esp /home/esp/.espressif, /bin/chown -R esp\:esp /home/esp/.cargo' \
    > /etc/sudoers.d/esp-volumes && \
    chmod 0440 /etc/sudoers.d/esp-volumes

# Pre-create cargo registry / git cache directories for volume caching
RUN mkdir -p /home/esp/.cargo/registry /home/esp/.cargo/git && \
    chown -R esp:esp /home/esp/.cargo

USER esp
WORKDIR /workspace

# Source the toolchain env for build commands (non-interactive shells)
ENV LIBCLANG_PATH="/home/esp/.rustup/toolchains/esp/xtensa-esp32-elf-clang/esp-20.1.1_20250829/esp-clang/lib"
ENV PATH="/home/esp/.rustup/toolchains/esp/xtensa-esp-elf/esp-15.2.0_20250920/xtensa-esp-elf/bin:/home/esp/.cargo/bin:${PATH}"

# ── QEMU for ESP32-S3 emulation ──────────────────────────────
USER root
RUN apt-get update && apt-get install -y --no-install-recommends \
        libgcrypt20 \
        libglib2.0-0 \
        libpixman-1-0 \
        libslirp0 \
        libsdl2-2.0-0 \
    && rm -rf /var/lib/apt/lists/*

# ESP-IDF tools (qemu-xtensa + riscv32-esp-elf) are installed in postCreateCommand
# after the /home/esp/.espressif volume is mounted and IDF_PATH is valid.
RUN echo "ESP-IDF tools install deferred to postCreateCommand"

# Install just (task runner)
RUN curl --proto '=https' --tlsv1.2 -sSf https://just.systems/install.sh | bash -s -- --to /usr/local/bin

USER esp
