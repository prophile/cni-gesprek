#!/bin/bash

set -e

# CNI-Gesprek Kubernetes Deployment Script
# This script deploys the CNI plugin to a Kubernetes cluster

NAMESPACE="${NAMESPACE:-kube-system}"
KUBECTL_CMD="${KUBECTL_CMD:-kubectl}"
IMAGE_NAME="${IMAGE_NAME:-cni-gesprek:latest}"
POD_CIDR="${POD_CIDR:-2001:db8:42::/48}"
MASTER_INTERFACE="${MASTER_INTERFACE:-}"

echo "Deploying CNI-Gesprek to Kubernetes cluster..."
echo "Namespace: $NAMESPACE"
echo "Image: $IMAGE_NAME"
echo "Pod CIDR: $POD_CIDR"

# Check if kubectl is available
if ! command -v "$KUBECTL_CMD" &> /dev/null; then
    echo "ERROR: $KUBECTL_CMD command not found. Please install kubectl or set KUBECTL_CMD environment variable."
    exit 1
fi

# Check cluster connectivity
if ! "$KUBECTL_CMD" cluster-info > /dev/null 2>&1; then
    echo "ERROR: Cannot connect to Kubernetes cluster. Please check your kubeconfig."
    exit 1
fi

echo "✓ Connected to Kubernetes cluster"

# Create namespace if it doesn't exist
if ! "$KUBECTL_CMD" get namespace "$NAMESPACE" > /dev/null 2>&1; then
    echo "Creating namespace $NAMESPACE..."
    "$KUBECTL_CMD" create namespace "$NAMESPACE"
fi

# Update ConfigMap with custom settings if provided
TEMP_DIR=$(mktemp -d)
trap "rm -rf $TEMP_DIR" EXIT

cp "$(dirname "$0")/../k8s/daemonset.yaml" "$TEMP_DIR/daemonset.yaml"

# Update image name if provided
if [[ "$IMAGE_NAME" != "cni-gesprek:latest" ]]; then
    sed -i "s|image: cni-gesprek:latest|image: $IMAGE_NAME|g" "$TEMP_DIR/daemonset.yaml"
    echo "✓ Updated image to $IMAGE_NAME"
fi

# Update pod CIDR if provided
if [[ "$POD_CIDR" != "2001:db8:42::/48" ]]; then
    sed -i "s|\"pod_cidr\": \"2001:db8:42::/48\"|\"pod_cidr\": \"$POD_CIDR\"|g" "$TEMP_DIR/daemonset.yaml"
    echo "✓ Updated pod CIDR to $POD_CIDR"
fi

# Update master interface if provided
if [[ -n "$MASTER_INTERFACE" ]]; then
    sed -i "s|\"master\": \"\"|\"master\": \"$MASTER_INTERFACE\"|g" "$TEMP_DIR/daemonset.yaml"
    echo "✓ Updated master interface to $MASTER_INTERFACE"
fi

# Deploy to cluster
echo "Applying CNI-Gesprek DaemonSet..."
"$KUBECTL_CMD" apply -f "$TEMP_DIR/daemonset.yaml" -n "$NAMESPACE"

echo "✓ CNI-Gesprek DaemonSet applied successfully"

# Wait for rollout
echo "Waiting for DaemonSet rollout..."
if "$KUBECTL_CMD" rollout status daemonset/cni-gesprek -n "$NAMESPACE" --timeout=300s; then
    echo "✓ DaemonSet rolled out successfully"
else
    echo "WARNING: DaemonSet rollout may still be in progress"
fi

# Show status
echo ""
echo "Deployment status:"
"$KUBECTL_CMD" get daemonset cni-gesprek -n "$NAMESPACE"
echo ""
echo "Pod status:"
"$KUBECTL_CMD" get pods -l app=cni-gesprek -n "$NAMESPACE"

echo ""
echo "CNI-Gesprek deployment completed!"
echo "You can check the logs with: $KUBECTL_CMD logs -l app=cni-gesprek -n $NAMESPACE"
