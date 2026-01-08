# Multi-stage build for CNI-Gesprek
# Build stage
FROM rust:1.75 AS builder

WORKDIR /usr/src/app

# Copy the entire source code
COPY . .

# Build the release binary
RUN cargo build --release

# Runtime stage
FROM ubuntu:22.04

# Install runtime dependencies
RUN apt-get update && \
    apt-get install -y \
    iproute2 \
    jq \
    iputils-ping \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create a non-root user (though CNI operations require root)
RUN groupadd -r cni && useradd -r -g cni cni

# Copy the binary from builder stage
COPY --from=builder /usr/src/app/target/release/cni-gesprek /usr/local/bin/cni-gesprek

# Copy installation scripts
COPY docker/install-cni.sh /install-cni.sh
COPY docker/uninstall-cni.sh /uninstall-cni.sh

# Make scripts executable
RUN chmod +x /install-cni.sh /uninstall-cni.sh /usr/local/bin/cni-gesprek

# Verify the binary works
RUN /usr/local/bin/cni-gesprek --help || true

# Set labels for better maintainability
LABEL maintainer="CNI-Gesprek Team" \
      version="1.0.0" \
      description="IPv6-only CNI plugin with cryptographically random IP addresses" \
      source="https://github.com/prophile/cni-gesprek"

# Default command runs the installation script
CMD ["/install-cni.sh"]
