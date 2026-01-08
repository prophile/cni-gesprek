# CNI Gesprek AI Coding Instructions

`cni-gesprek` is an IPv6-only CNI plugin written in Rust that creates `ipvlan` interfaces with cryptographically random IP addresses. This plugin emphasizes zero-config operation, strict collision detection, and synchronous I/O.

## Architecture Overview

The codebase follows a **layered architecture** with clear separation between business logic and system operations:

- **Orchestrator Layer** ([`orchestrator.rs`](src/orchestrator.rs)): Pure business logic for network configuration planning, completely testable without system dependencies
- **Driver Layer** ([`driver.rs`](src/driver.rs)): Interface abstractions for network operations (`NetworkDiscovery`, `InterfaceLifecycle`, `AddressConfiguration`)
- **Implementation Layer**: Concrete drivers like [`ScriptDriver`](src/driver_script.rs) (shell commands) and [`DryRunDriver`](src/driver_dryrun.rs) (testing)
- **Command Layer** ([`command_dispatcher.rs`](src/command_dispatcher.rs)): CNI command routing with consolidated context handling

## Core Design Principles

### 1. Testability Through Abstraction
All business logic is driver-agnostic. The [`CommandCaptureDriver`](src/testing/command_capture.rs) captures executed commands for white-box testing:

```rust
// Test network commands without root privileges
let driver = CommandCaptureDriver::new();
// ... execute operations ...
assert!(driver.has_command("ip", &["link", "add"]));
```

### 2. Context Consolidation Pattern
Use [`CommandContext`](src/command_dispatcher.rs) instead of scattered parameters:

```rust
let ctx = CommandContext::new(args, cni_context, output);
ctx.require_config()?; // Consistent error handling
```

### 3. Structured Error Hierarchy
Follow the [`CniError`](src/error.rs) hierarchy: `Config` → `Environment` → `Command` errors with specific error types for each domain.

## Development Workflows

### Testing Strategy
- **Unit tests**: Individual components using mock drivers
- **White-box tests**: Using [`CommandCaptureDriver`](src/testing/command_capture.rs) to validate network commands
- **Integration tests**: [`tests/`](tests/) directory with real binary execution
- **Performance tests**: [`benches/`](benches/) for IP generation algorithms

Run tests with network isolation:
```bash
cargo test --features testing  # Enable test-only modules
cargo test --test integration_tests  # Integration tests
cargo bench  # Performance benchmarks
```

### Build Commands
```bash
cargo build --release  # Production CNI binary
cargo build --features testing  # Include testing modules
```

## Project-Specific Conventions

### 1. IPv6 Type Safety
Use [`Ipv6Subnet`](src/driver.rs) struct instead of string parsing for subnet operations. The [`utils::generate_random_ip`](src/lib.rs) function handles cryptographic randomness with proper CIDR validation.

### 2. Driver Pattern Implementation
When adding new drivers:
- Implement [`NetworkDriver`](src/driver.rs) trait combining all network operations
- Register in [`DriverFactory`](src/driver_factory.rs)
- Add corresponding test driver in [`testing/`](src/testing/) module

### 3. CNI Protocol Handling
CNI context loading follows strict patterns in [`CniContext::load`](src/cni.rs):
- Environment variables → Configuration parsing → Validation
- CLI overrides take precedence over config file values
- Dry-run mode affects driver selection, not business logic

### 4. Configuration Builder Pattern
For tests, use [`CniConfigBuilder`](src/testing/config_builder.rs) and [`CniEnvBuilder`](src/testing/config_builder.rs) for consistent test setups:

```rust
let config = CniConfigBuilder::new()
    .with_name("test-net")
    .with_master("eth0")
    .with_pod_cidr("2001:db8::/64")
    .build();
```

## Key Integration Points

- **Environment Variables**: [`environment.rs`](src/environment.rs) provides trait-based environment access for testing
- **Output Writing**: [`output.rs`](src/output.rs) abstracts JSON output for testability  
- **Command Execution**: System commands are abstracted through driver traits, never executed directly in business logic
- **Random Generation**: Uses `rand` crate with proper IPv6 subnet arithmetic in [`utils`](src/lib.rs)

## Testing Integration
The [`testing`](src/testing/) module is feature-gated (`#[cfg(any(test, feature = "testing"))]`) and provides comprehensive test infrastructure. Always use test builders and command capture for consistent, isolated testing.

## Completion

Code is not complete until:

- All new features have test coverage,
- All tests pass (across the board),
- `cargo clippy` reports no errors or warnings,
- `TODO.md` is updated with any new technical debt or improvements identified during development,
- `TODO.md` status is changed to "Done" for completed tasks.
