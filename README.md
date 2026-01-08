# cni-gesprek

`cni-gesprek` is a lightweight, IPv6-only Container Network Interface (CNI) plugin written in Rust.

It provides container connectivity by creating an `ipvlan` interface (L2 mode, bridge). Instead of relying on DHCP or kernel SLAAC, it instantaneously assigns a **random, cryptographically generated IPv6 address** within the host's subnet. It also explicitly copies the default gateway from the host to the container, ensuring immediate connectivity without the latency of waiting for Router Advertisements.

## Features

* **IPv6 Native:** Built strictly for single-stack IPv6 environments.
* **Pseudo-Stateless:** mimics "stateless" auto-configuration by generating random stable addresses, but applied statically for speed.
* **Zero-Config:** Automatically detects the upstream interface, subnet, and gateway from the host's routing table.
* **Collision Safe:** Performs strict checks on Duplicate Address Detection (DAD) status, failing fast if a collision occurs (allowing the orchestrator to retry).
* **No Async Runtime:** Uses synchronous I/O for small binary size and simple execution flow.

## Installation

```bash
cargo build --release
sudo cp target/release/cni-gesprek /opt/cni/bin/
```

## Configuration Example

`/etc/cni/net.d/10-gesprek.conf`:

```json
{
  "cniVersion": "1.0.0",
  "name": "mynet",
  "type": "gesprek"
}
```

## Manual Override

```json
{
  "cniVersion": "1.0.0",
  "name": "mynet",
  "type": "gesprek",
  "master": "eth1",
  "podCIDR": "2001:db8:cafe::/64"
}
```

## Kubernetes Deployment

### Quick Start

To deploy CNI-Gesprek to a Kubernetes cluster:

```bash
# Build and push the Docker image
docker build -t your-registry/cni-gesprek:latest .
docker push your-registry/cni-gesprek:latest

# Deploy to cluster
./scripts/deploy-k8s.sh
```

### Prerequisites

- Kubernetes cluster with IPv6 support enabled
- `kubectl` configured to access your cluster
- Docker registry accessible by cluster nodes

### Building the Container Image

Two Dockerfile variants are available:

**Standard Ubuntu-based image:**
```bash
docker build -f docker/Dockerfile -t cni-gesprek:latest .
```

**Minimal Alpine-based image:**
```bash
docker build -f docker/Dockerfile.alpine -t cni-gesprek:alpine .
```

### Configuration

Before deploying, customize the CNI configuration in [`k8s/daemonset.yaml`](k8s/daemonset.yaml):

```yaml
# Update the ConfigMap section
data:
  cni_network_config: |
    {
      "cniVersion": "1.0.0",
      "name": "cni-gesprek",
      "type": "cni-gesprek",
      "master": "eth0",                    # Host interface (empty for auto-detection)
      "pod_cidr": "2001:db8:42::/48",      # IPv6 subnet for pods
      "mtu": 1500,
      "dns": {
        "nameservers": ["2001:4860:4860::8888", "2001:4860:4860::8844"]
      }
    }
```

### Deployment Options

**Option 1: Automated deployment script**
```bash
# Basic deployment
./scripts/deploy-k8s.sh

# With custom settings
NAMESPACE=kube-system \
IMAGE_NAME=your-registry/cni-gesprek:latest \
POD_CIDR=2001:db8:cafe::/48 \
MASTER_INTERFACE=ens3 \
./scripts/deploy-k8s.sh
```

**Option 2: Manual deployment**
```bash
# Apply directly
kubectl apply -f k8s/daemonset.yaml

# Monitor deployment
kubectl rollout status daemonset/cni-gesprek -n kube-system
```

### Verification

Check that the CNI plugin is running on all nodes:

```bash
# Check DaemonSet status
kubectl get daemonset cni-gesprek -n kube-system

# Check pod logs
kubectl logs -l app=cni-gesprek -n kube-system

# Verify CNI binary installation
kubectl exec -n kube-system cni-gesprek-xxxx -- ls -la /opt/cni/bin/cni-gesprek
```

Test with a sample pod:

```yaml
apiVersion: v1
kind: Pod
metadata:
  name: test-ipv6
spec:
  containers:
  - name: test
    image: alpine:latest
    command: ["sleep", "3600"]
```

```bash
kubectl apply -f test-pod.yaml
kubectl exec test-ipv6 -- ip -6 addr show
```

### Troubleshooting

**Common Issues:**

1. **DaemonSet pods failing to start:**
   - Check node taints and tolerations
   - Verify image pull permissions
   - Check logs: `kubectl logs -l app=cni-gesprek -n kube-system`

2. **CNI configuration errors:**
   - Validate JSON in ConfigMap: `kubectl get configmap cni-gesprek-config -n kube-system -o yaml`
   - Check CNI directory permissions on nodes

3. **Network connectivity issues:**
   - Verify IPv6 is enabled on cluster nodes
   - Check routing configuration: `ip -6 route show`
   - Verify master interface exists and has IPv6 address

**Debug mode:**
```bash
# Enable debug logging in DaemonSet
kubectl patch daemonset cni-gesprek -n kube-system -p '{"spec":{"template":{"spec":{"containers":[{"name":"install-cni","env":[{"name":"SLEEP","value":"true"}]}]}}}}'

# Access debug shell
kubectl exec -n kube-system -it cni-gesprek-xxxx -- /bin/bash
```

### Uninstallation

Remove CNI-Gesprek from the cluster:

```bash
kubectl delete -f k8s/daemonset.yaml
```

The uninstall script will automatically clean up CNI binaries and configuration from all nodes.

## License

This project is licensed under the *CC0 1.0 Universal (CC0-1.0) Public Domain Dedication*.
