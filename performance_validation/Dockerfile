# Seen Language Performance Validation - Reproducible Docker Environment
FROM ubuntu:22.04

# Prevent interactive prompts
ENV DEBIAN_FRONTEND=noninteractive

# Install base dependencies
RUN apt-get update && apt-get install -y \
    build-essential \
    cmake \
    git \
    curl \
    wget \
    python3 \
    python3-pip \
    clang-14 \
    llvm-14 \
    lld-14 \
    time \
    valgrind \
    perf-tools-unstable \
    linux-tools-generic \
    hyperfine \
    && rm -rf /var/lib/apt/lists/*

# Install Rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

# Install Zig
RUN wget https://ziglang.org/download/0.11.0/zig-linux-x86_64-0.11.0.tar.xz \
    && tar -xf zig-linux-x86_64-0.11.0.tar.xz \
    && mv zig-linux-x86_64-0.11.0 /opt/zig \
    && ln -s /opt/zig/zig /usr/local/bin/zig \
    && rm zig-linux-x86_64-0.11.0.tar.xz

# Install Go (for additional benchmarks)
RUN wget https://go.dev/dl/go1.21.5.linux-amd64.tar.gz \
    && tar -C /usr/local -xzf go1.21.5.linux-amd64.tar.gz \
    && rm go1.21.5.linux-amd64.tar.gz
ENV PATH="/usr/local/go/bin:${PATH}"

# Install Python packages for analysis
RUN pip3 install --no-cache-dir \
    numpy \
    scipy \
    matplotlib \
    pandas \
    seaborn \
    plotly \
    jupyterlab

# Create working directory
WORKDIR /validation

# Copy validation framework
COPY . /validation/

# Install Seen compiler (when available)
# For now, we'll build a placeholder
RUN echo "#!/bin/bash\necho 'Seen compiler placeholder'" > /usr/local/bin/seen \
    && chmod +x /usr/local/bin/seen

# Set up validation environment
ENV VALIDATION_HOME=/validation
ENV RESULTS_DIR=/validation/results
ENV HONEST_MODE=true

# Create entrypoint script
RUN echo '#!/bin/bash\n\
echo "=================================================="\n\
echo " Seen Language Performance Validation Environment"\n\
echo "=================================================="\n\
echo ""\n\
echo "This Docker container provides a reproducible environment for"\n\
echo "third-party validation of Seen language performance claims."\n\
echo ""\n\
echo "Available commands:"\n\
echo "  validate-all       - Run complete validation suite"\n\
echo "  validate-lexer     - Validate lexer performance"\n\
echo "  validate-parser    - Validate parser performance"\n\
echo "  validate-reactive  - Validate reactive zero-cost claims"\n\
echo "  validate-memory    - Validate memory usage claims"\n\
echo "  generate-report    - Generate comprehensive report"\n\
echo ""\n\
echo "All results are saved to /validation/results/"\n\
echo ""\n\
\n\
if [ "$1" = "validate-all" ]; then\n\
    echo "Starting complete validation suite..."\n\
    cd /validation\n\
    ./scripts/validate_all.sh\n\
elif [ "$1" = "validate-lexer" ]; then\n\
    echo "Validating lexer performance..."\n\
    cd /validation/benchmarks/lexer\n\
    ./run_real_benchmark.sh\n\
elif [ "$1" = "validate-parser" ]; then\n\
    echo "Validating parser performance..."\n\
    cd /validation/benchmarks/parser\n\
    ./run_real_benchmark.sh\n\
elif [ "$1" = "validate-reactive" ]; then\n\
    echo "Validating reactive zero-cost claims..."\n\
    cd /validation/benchmarks/reactive\n\
    ./run_real_benchmark.sh\n\
elif [ "$1" = "validate-memory" ]; then\n\
    echo "Validating memory usage..."\n\
    cd /validation/benchmarks/memory\n\
    ./run_real_benchmark.sh\n\
elif [ "$1" = "generate-report" ]; then\n\
    echo "Generating validation report..."\n\
    cd /validation\n\
    python3 scripts/report_generator.py --data-dir results/latest --output results/validation_report.md --format markdown --honest-mode\n\
else\n\
    exec "$@"\n\
fi\n\
' > /usr/local/bin/docker-entrypoint.sh \
    && chmod +x /usr/local/bin/docker-entrypoint.sh

ENTRYPOINT ["/usr/local/bin/docker-entrypoint.sh"]
CMD ["bash"]