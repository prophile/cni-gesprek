#!/bin/bash

set -e

# CNI-Gesprek Installation Script for Kubernetes
# This script is run by the DaemonSet to install the CNI plugin and configuration

# Default values
CNI_NET_DIR="${CNI_NET_DIR:-/etc/cni/net.d}"
CNI_BIN_DIR="${CNI_BIN_DIR:-/opt/cni/bin}"
CNI_CONF_NAME="${CNI_CONF_NAME:-10-cni-gesprek.conflist}"
SLEEP="${SLEEP:-true}"

echo "Starting CNI-Gesprek installation..."
echo "CNI_NET_DIR: $CNI_NET_DIR"
echo "CNI_BIN_DIR: $CNI_BIN_DIR"
echo "CNI_CONF_NAME: $CNI_CONF_NAME"

# Function to check if directory exists and is writable
check_directory() {
    local dir=$1
    local name=$2
    
    if [[ ! -d "$dir" ]]; then
        echo "ERROR: $name directory $dir does not exist"
        exit 1
    fi
    
    if [[ ! -w "$dir" ]]; then
        echo "ERROR: $name directory $dir is not writable"
        exit 1
    fi
    
    echo "✓ $name directory $dir is accessible"
}

# Check required directories
check_directory "$CNI_NET_DIR" "CNI configuration"
check_directory "$CNI_BIN_DIR" "CNI binary"

# Copy CNI binary
echo "Installing CNI binary..."
cp /usr/local/bin/cni-gesprek "$CNI_BIN_DIR/"
chmod +x "$CNI_BIN_DIR/cni-gesprek"
echo "✓ CNI binary installed to $CNI_BIN_DIR/cni-gesprek"

# Install CNI configuration
echo "Installing CNI configuration..."
if [[ -n "$CNI_NETWORK_CONFIG" ]]; then
    echo "$CNI_NETWORK_CONFIG" > "$CNI_NET_DIR/$CNI_CONF_NAME"
    echo "✓ CNI configuration installed to $CNI_NET_DIR/$CNI_CONF_NAME"
else
    echo "ERROR: CNI_NETWORK_CONFIG environment variable not set"
    exit 1
fi

# Validate configuration
echo "Validating CNI configuration..."
if ! echo "$CNI_NETWORK_CONFIG" | jq . > /dev/null 2>&1; then
    echo "ERROR: CNI configuration is not valid JSON"
    exit 1
fi

# Check required fields in configuration
if ! echo "$CNI_NETWORK_CONFIG" | jq -e '.name' > /dev/null; then
    echo "ERROR: CNI configuration missing required 'name' field"
    exit 1
fi

if ! echo "$CNI_NETWORK_CONFIG" | jq -e '.type' > /dev/null; then
    echo "ERROR: CNI configuration missing required 'type' field"
    exit 1
fi

if ! echo "$CNI_NETWORK_CONFIG" | jq -e '.pod_cidr' > /dev/null; then
    echo "ERROR: CNI configuration missing required 'pod_cidr' field"
    exit 1
fi

echo "✓ CNI configuration validated successfully"

# Test basic CNI binary functionality
echo "Testing CNI binary..."
if ! "$CNI_BIN_DIR/cni-gesprek" --help > /dev/null 2>&1; then
    echo "WARNING: CNI binary test failed - this may be expected in some environments"
else
    echo "✓ CNI binary test passed"
fi

echo "CNI-Gesprek installation completed successfully!"

# Keep the container alive if requested (for debugging)
if [[ "$SLEEP" == "true" ]]; then
    echo "Entering sleep mode for debugging..."
    while true; do
        sleep 3600
    done
fi
