# CNI Gesprek Testing Framework

This directory contains a comprehensive white-box testing framework for the CNI Gesprek plugin.

## Overview

The testing framework provides multiple levels of testing:

1. **Unit Tests**: Test individual components and functions
2. **White-box Tests**: Test the binary with captured command execution
3. **Integration Tests**: End-to-end tests using the actual binary
4. **Command Validation**: Verify that the correct network commands are generated

## Components

### Command Capture Driver (`command_capture.rs`)

A test driver that captures all network commands that would be executed instead of running them. This allows us to:

- Verify command correctness without requiring root privileges
- Test networking logic without actual network interfaces
- Validate command sequences and parameters
- Mock network responses for consistent testing

Example usage:
```rust
let driver = CommandCaptureDriver::new();
// ... execute CNI operations ...
let commands = driver.captured_commands();
assert!(driver.has_command("ip", &["link", "add"]));
```

### Configuration Builder (`config_builder.rs`)

Builders for creating CNI configurations and environment setups:

```rust
let config = CniConfigBuilder::new()
    .with_name("test-net")
    .with_master("eth0")
    .with_pod_cidr("2001:db8::/64")
    .build();

let env = CniEnvBuilder::new()
    .with_command("ADD")
    .with_netns("/proc/123/ns/net")
    .with_ifname("eth0")
    .build();
```

### Test Runner (`test_runner.rs`)

Core test execution engine that:

- Runs the CNI binary with specific configurations
- Captures and parses command outputs
- Provides validation helpers
- Supports both dry-run and actual execution modes

### Test Cases (`test_cases.rs`)

Pre-built test cases covering:

- Basic ADD operations
- DNS configuration handling
- Interface auto-detection
- Subnet auto-detection
- All CNI commands (ADD, DEL, CHECK, VERSION, STATUS, GC)
- Error conditions and edge cases

## Running Tests

### 1. Unit Tests
```bash
cargo test
```

### 2. Integration Tests
```bash
cargo test --test integration_tests
```

### 3. White-box Test Runner
```bash
# Build first
cargo build --release

# Run white-box tests
cargo run --bin test-runner target/release/cni-gesprek
```

### 4. Dry Run Testing
```bash
# Test with dry run mode
echo '{"cniVersion":"1.0.0","name":"test","type":"cni-gesprek","master":"eth0"}' | \
  CNI_COMMAND=ADD CNI_NETNS=/proc/123/ns/net CNI_IFNAME=eth0 \
  target/release/cni-gesprek --dry-run
```

## Test Scenarios

The framework includes several standard test scenarios:

### Basic ADD Command
Tests the fundamental ADD operation with:
- Interface specification
- CIDR configuration
- Network namespace setup
- IP address assignment
- Gateway configuration

### Auto-detection Tests
- **Interface Detection**: Test automatic detection of upstream interface via routing table
- **Subnet Detection**: Test automatic subnet discovery from interface addresses

### DNS Configuration
Tests DNS configuration pass-through in CNI results

### Error Conditions
- Missing environment variables
- Invalid JSON configuration
- Unknown CNI commands
- Network interface errors

## Validation Framework

The testing framework provides comprehensive validation through `CommandValidators`:

### Command Validation
```rust
// Validate specific commands were executed
CommandValidators::expect_command(&mut result, "ip", &["link", "show"], "Interface check");

// Validate nsenter commands for namespace operations
CommandValidators::expect_nsenter_command(&mut result, "/proc/123/ns/net", 
    &["ip", "addr", "add"], "IP assignment");

// Validate JSON output structure
CommandValidators::expect_json_output(&mut result, &["interfaces", "ips"], "CNI result");
```

### Command Ordering
```rust
// Validate commands execute in correct order
CommandValidators::expect_command_order(&mut result, &[
    ("ip".to_string(), vec!["link".to_string(), "add".to_string()]),
    ("ip".to_string(), vec!["link".to_string(), "set".to_string()]),
], "IPvlan creation sequence");
```

### IPvlan-specific Validation
```rust
// Validate IPvlan creation with correct parameters
CommandValidators::expect_ipvlan_creation(&mut result, "eth0", "l2", "IPvlan setup");
```

## Expected Command Sequences

For a typical ADD operation, the framework validates this command sequence:

1. **Interface Detection** (if not specified):
   ```bash
   ip -6 -j route show default
   ```

2. **Interface Validation**:
   ```bash
   ip link show dev eth0
   ```

3. **Subnet Detection** (if not specified):
   ```bash
   ip -j -6 addr show dev eth0 scope global
   ```

4. **Gateway Detection**:
   ```bash
   ip -6 -j route show default dev eth0
   ```

5. **IPvlan Creation**:
   ```bash
   ip link add link eth0 name ipvlXXXXXXXX type ipvlan mode l2 bridge
   ```

6. **Namespace Assignment**:
   ```bash
   ip link set dev ipvlXXXXXXXX netns /proc/123/ns/net
   ```

7. **In-namespace Configuration**:
   ```bash
   nsenter --net=/proc/123/ns/net -F -- ip link set dev ipvlXXXXXXXX name eth0
   nsenter --net=/proc/123/ns/net -F -- ip addr add 2001:db8::XXXX/64 dev eth0
   nsenter --net=/proc/123/ns/net -F -- sysctl -w net.ipv6.conf.eth0.accept_ra=2
   nsenter --net=/proc/123/ns/net -F -- ip link set up dev eth0
   nsenter --net=/proc/123/ns/net -F -- ip -6 route add default via fe80::1 dev eth0
   nsenter --net=/proc/123/ns/net -F -- ip -j -6 addr show dev eth0
   ```

## Extending the Framework

To add new test cases:

1. **Create a Test Case**:
```rust
pub struct MyCustomTest {
    pub name: String,
    pub config: CniConfigBuilder,
    pub env: CniEnvBuilder,
}

impl TestCase for MyCustomTest {
    fn name(&self) -> &str { &self.name }
    
    fn setup(&mut self) -> Result<(), Box<dyn Error>> { Ok(()) }
    
    fn execute(&mut self, runner: &TestRunner) -> Result<TestResult, Box<dyn Error>> {
        runner.run_cni(&self.config, &self.env, true)
    }
    
    fn validate(&self, result: &mut TestResult) {
        // Add custom validations
    }
}
```

2. **Add to Test Suite**:
```rust
let test_cases: Vec<Box<dyn TestCase>> = vec![
    // existing tests...
    Box::new(MyCustomTest::new()),
];
```

## Continuous Integration

The framework is designed for CI/CD integration:

```yaml
# Example GitHub Actions workflow
- name: Run Tests
  run: |
    cargo build --release
    cargo test
    cargo run --bin test-runner target/release/cni-gesprek
```

## Debugging

When tests fail, the framework provides detailed output:

- **Command Capture**: See exactly what commands would be executed
- **JSON Validation**: Verify CNI result structure
- **Error Messages**: Specific validation failure details
- **Exit Codes**: Process return code validation

Enable debug output:
```bash
RUST_LOG=debug cargo test
```

## Architecture Benefits

This white-box testing approach provides:

1. **No Root Required**: Tests run without privileged access
2. **Fast Execution**: No actual network operations
3. **Deterministic**: Consistent results across environments  
4. **Comprehensive**: Tests both success and failure paths
5. **Maintainable**: Easy to add new test scenarios
6. **CI-Friendly**: Integrates well with automated testing pipelines

The framework ensures that the CNI plugin generates the correct network configuration commands without requiring actual network interfaces or root privileges, making it ideal for development and CI environments.
