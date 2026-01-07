#!/bin/bash

# Build the project
echo "Building CNI Gesprek..."
cargo build --release

if [ $? -ne 0 ]; then
    echo "Build failed!"
    exit 1
fi

# Run unit tests
echo -e "\n=== Running Unit Tests ==="
cargo test --lib

# Run integration tests 
echo -e "\n=== Running Integration Tests ==="
cargo test --test integration_tests

# Run white-box tests
echo -e "\n=== Running White-box Tests ==="
cargo run --bin test-runner target/release/cni-gesprek

# Test a few manual examples
echo -e "\n=== Manual Test Examples ==="

echo "Testing VERSION command:"
CNI_COMMAND=VERSION ./target/release/cni-gesprek

echo -e "\nTesting ADD command (dry run):"
echo '{"cniVersion":"1.0.0","name":"test","type":"cni-gesprek","master":"eth0","podCIDR":"2001:db8::/64"}' | \
  CNI_COMMAND=ADD CNI_NETNS=/proc/123/ns/net CNI_IFNAME=eth0 CNI_CONTAINERID=test123 \
  ./target/release/cni-gesprek --dry-run

echo -e "\nTesting DEL command:"
echo '{"cniVersion":"1.0.0","name":"test","type":"cni-gesprek"}' | \
  CNI_COMMAND=DEL CNI_NETNS=/proc/123/ns/net CNI_IFNAME=eth0 CNI_CONTAINERID=test123 \
  ./target/release/cni-gesprek

echo -e "\nAll tests completed!"
