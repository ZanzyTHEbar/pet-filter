# PetFilter KiCad Development Environment
# Based on KiBot - the standard KiCad automation tool
# Provides: KiCad CLI, KiBot, InteractiveHtmlBom, pcbdraw, blender
FROM ghcr.io/inti-cmnb/kicad8_auto:latest

LABEL maintainer="PetFilter Engineering"
LABEL description="KiCad 8 + KiBot automation for PetFilter PCB design"

# Install additional tools
RUN apt-get update && apt-get install -y --no-install-recommends \
    python3-pip \
    make \
    git \
    jq \
    && rm -rf /var/lib/apt/lists/*

# Set working directory
WORKDIR /workspace

# Default command: run KiBot
ENTRYPOINT ["/bin/bash"]
