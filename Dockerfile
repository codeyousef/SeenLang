# Use the official Rust image as a base.
FROM rust:1.78-bookworm

# Set a working directory in the container
WORKDIR /usr/src/app

# Set DEBIAN_FRONTEND to noninteractive to avoid prompts during package installation
ENV DEBIAN_FRONTEND=noninteractive

# Install essential Linux packages and prerequisites for LLVM
# *** Added lsb-release AND software-properties-common here ***
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    curl \
    wget \
    gnupg \
    ca-certificates \
    lsb-release \
    software-properties-common \
    git \
    pkg-config \
    libssl-dev \
    cmake \
    ninja-build \
    python3 \
    python3-pip \
    && rm -rf /var/lib/apt/lists/*

# Add LLVM APT repository and install LLVM 18 components using the official script
# Also install dev packages and lldb
RUN wget https://apt.llvm.org/llvm.sh -O /tmp/llvm.sh && \
    chmod +x /tmp/llvm.sh && \
    # Execute the script to add repo and install specific version 18 runtime/tools
    # This handles the GPG key and sources list addition.
    # We explicitly list components to avoid installing 'polly' by default.
    /tmp/llvm.sh 18 llvm clang lld lldb && \
    # Install development files and lldb specifically after adding the repo
    apt-get update && \
    apt-get install -y --no-install-recommends \
        llvm-18-dev \
        libclang-18-dev \
        lldb-18 \
    && \
    # Clean up
    rm /tmp/llvm.sh && \
    rm -rf /var/lib/apt/lists/*

# Explicitly add the LLVM 18 bin directory to the PATH environment variable
# The llvm.sh script *should* set up alternatives, but this is a fallback.
ENV PATH="/usr/lib/llvm-18/bin:${PATH}"

# Verify installation during build (optional but helpful for debugging)
RUN echo "Verifying LLVM installation..." && \
    ls -l /usr/bin/llvm-config* || true && \
    ls -l /usr/lib/llvm-18/bin/llvm-config && \
    llvm-config --version && \
    clang --version

# Install common Rust tools
RUN rustup component add clippy
RUN rustup component add rustfmt
RUN rustup component add rust-analyzer

# (Optional) Create non-root user
# ARG USERNAME=vscode
# ARG USER_UID=1000
# ARG USER_GID=$USER_UID
# RUN groupadd --gid $USER_GID $USERNAME \
#     && useradd --uid $USER_UID --gid $USER_GID -m $USERNAME \
#     && apt-get update && apt-get install -y sudo \
#     && echo $USERNAME ALL=\(root\) NOPASSWD:ALL > /etc/sudoers.d/$USERNAME \
#     && chmod 0440 /etc/sudoers.d/$USERNAME \
#     && rm -rf /var/lib/apt/lists/*

# Switch to the non-root user if created
# USER $USERNAME

# Default command
CMD ["bash"]