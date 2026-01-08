#!/bin/bash

set -e

# CNI-Gesprek Uninstallation Script for Kubernetes
# This script is run by the DaemonSet during pod termination

# Default values
CNI_NET_DIR="${CNI_NET_DIR:-/etc/cni/net.d}"
CNI_BIN_DIR="${CNI_BIN_DIR:-/opt/cni/bin}"
CNI_CONF_NAME="${CNI_CONF_NAME:-10-cni-gesprek.conflist}"

echo "Starting CNI-Gesprek uninstallation..."

# Remove CNI configuration file
if [[ -f "$CNI_NET_DIR/$CNI_CONF_NAME" ]]; then
    rm -f "$CNI_NET_DIR/$CNI_CONF_NAME"
    echo "✓ Removed CNI configuration: $CNI_NET_DIR/$CNI_CONF_NAME"
else
    echo "CNI configuration not found: $CNI_NET_DIR/$CNI_CONF_NAME"
fi

# Remove CNI binary
if [[ -f "$CNI_BIN_DIR/cni-gesprek" ]]; then
    rm -f "$CNI_BIN_DIR/cni-gesprek"
    echo "✓ Removed CNI binary: $CNI_BIN_DIR/cni-gesprek"
else
    echo "CNI binary not found: $CNI_BIN_DIR/cni-gesprek"
fi

# Clean up any leftover network state (best effort)
# Note: In production, you might want to be more careful about cleanup
# to avoid disrupting running pods
if [[ -d "/var/lib/cni/networks/cni-gesprek" ]]; then
    rm -rf "/var/lib/cni/networks/cni-gesprek"
    echo "✓ Cleaned up network state directory"
fi

echo "CNI-Gesprek uninstallation completed"
