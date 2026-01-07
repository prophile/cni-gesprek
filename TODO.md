# CNI-Gesprek Code Review TODO

## Critical Issues

### 1. Naming Convention Violations
**Priority**: High
**Status**: Open
**Location:** [src/main.rs](src/main.rs#L25), [src/main.rs](src/main.rs#L34), [src/main.rs](src/main.rs#L54)  
**Description:** Struct fields use `camelCase` instead of Rust's `snake_case` convention (e.g., `cniVersion`, `podCIDR`)  
**Recommended Actions:**
- Add proper serde annotations to maintain JSON compatibility: `#[serde(rename = "cniVersion")]`
- Rename fields to follow Rust conventions: `cni_version`, `pod_cidr`
- Update all references to use new field names

### 2. Unsafe Random IP Generation
**Priority**: High
**Status**: Open
**Location:** [src/main.rs](src/main.rs#L85-L120)  
**Description:** Complex bit manipulation in `generate_random_ip` function is error-prone and may generate invalid addresses  
**Recommended Actions:**
- Simplify IP generation logic using proper IPv6 address libraries
- Add comprehensive test coverage for edge cases (prefix lengths 0, 64, 128)
- Validate generated IPs are within the expected subnet
- Consider using `ipnet` crate for proper CIDR handling

### 3. Error Handling Issues
**Priority**: High
**Status**: Open
**Location:** [src/main.rs](src/main.rs#L140)  
**Description:** Silent failures in stdin parsing and missing validation for critical environment variables  
**Recommended Actions:**
- Replace `.ok()` with proper error propagation for JSON parsing
- Add validation for `CNI_NETNS`, `CNI_IFNAME` before use
- Create structured error types for better error context
- Add logging for debugging failed configurations

## Security & Reliability Concerns

### 4. Command Injection Vulnerability
**Priority**: High
**Status**: Open
**Location:** [src/driver_script.rs](src/driver_script.rs#L10-L25)  
**Description:** No input sanitization for interface names or IP addresses passed to shell commands  
**Recommended Actions:**
- Implement input validation for interface names (Linux naming rules)
- Sanitize all user-provided strings before shell execution
- Use parameterized commands where possible
- Add regex validation for IP addresses and CIDR notation

### 5. Race Condition in DAD Check
**Priority**: High
**Status**: Open
**Location:** [src/driver_script.rs](src/driver_script.rs#L203-L259)  
**Description:** IPv6 Duplicate Address Detection polling has hardcoded timeouts and no proper synchronization  
**Recommended Actions:**
- Implement exponential backoff for DAD polling
- Make timeout configurable via environment variable
- Add better error messages for DAD failure scenarios
- Consider using netlink sockets instead of polling `ip` command

### 6. Resource Leak Risk
**Priority**: High
**Status**: Open
**Location:** [src/main.rs](src/main.rs#L210-L220)  
**Description:** Created ipvlan interfaces may not be cleaned up if namespace configuration fails  
**Recommended Actions:**
- Implement RAII pattern for network interface management
- Add defer-like cleanup using Drop trait
- Create rollback logic for failed ADD operations
- Add comprehensive error recovery paths

## Best Practices & Code Quality

### 7. Poor Error Messages
**Priority**: Medium
**Status**: Open
**Location:** [src/driver_script.rs](src/driver_script.rs#L15)  
**Description:** Generic error messages don't provide enough context for debugging  
**Recommended Actions:**
- Include command details and system state in error messages
- Add structured logging with different log levels
- Provide suggestions for common failure scenarios
- Include system information in error context

### 8. Magic Numbers
**Priority**: Medium
**Status**: Open
**Location:** [src/driver_script.rs](src/driver_script.rs#L203), [src/main.rs](src/main.rs#L206)  
**Description:** Hardcoded values throughout codebase (timeouts, string lengths)  
**Recommended Actions:**
- Define constants for all magic numbers
- Make timeouts configurable via environment variables
- Document rationale for chosen values
- Consider making random suffix length configurable

### 9. Code Duplication
**Priority**: Medium
**Status**: Open
**Location:** [src/driver_script.rs](src/driver_script.rs#L130-L200)  
**Description:** Repetitive nsenter command construction and JSON parsing patterns  
**Recommended Actions:**
- Extract common nsenter command building into helper function
- Create reusable JSON parsing utilities
- Implement builder pattern for complex command construction
- Consider using macros for repetitive command patterns

### 10. Missing Input Validation
**Priority**: Medium
**Status**: Open
**Location:** [src/main.rs](src/main.rs#L85), [src/main.rs](src/main.rs#L170)  
**Description:** No validation for CIDR format, interface names, or other inputs  
**Recommended Actions:**
- Implement comprehensive input validation functions
- Add regex patterns for Linux interface names
- Validate IPv6 addresses and CIDR notation
- Check environment variable formats before use

## Design Issues

### 11. Inconsistent Abstractions
**Priority**: Medium
**Status**: Open
**Location:** [src/driver.rs](src/driver.rs#L5-L45)  
**Description:** NetworkDriver trait mixes high-level and low-level operations inconsistently  
**Recommended Actions:**
- Separate high-level network operations from low-level system calls
- Make return types consistent (all use Result<T, E>)
- Consider splitting into multiple smaller traits
- Add trait documentation with usage examples

### 12. Tight Coupling
**Priority**: Medium
**Status**: Open
**Location:** [src/main.rs](src/main.rs#L125-L285)  
**Description:** Main function handles multiple concerns making testing difficult  
**Recommended Actions:**
- Extract CNI protocol handling into separate module
- Create testable business logic layer
- Implement dependency injection for drivers
- Separate CLI argument parsing from core logic

## Minor Issues

### 13. Missing Documentation
**Priority**: Low
**Status**: Open
**Location:** Throughout codebase  
**Description:** No doc comments for public APIs and complex algorithms  
**Recommended Actions:**
- Add comprehensive doc comments for all public functions
- Include usage examples for complex functions like `generate_random_ip`
- Document error conditions and edge cases
- Add module-level documentation explaining architecture

### 14. Dead Code and Placeholder Logic
**Priority**: Low
**Status**: Open
**Location:** [src/main.rs](src/main.rs#L268-L285)  
**Description:** Some command handlers return placeholder responses that may not be CNI-compliant  
**Recommended Actions:**
- Review CNI specification for proper response formats
- Implement proper DEL command logic for cleanup
- Add proper CHECK command validation
- Remove or properly implement placeholder responses

## Testing Improvements

### 15. Insufficient Integration Test Coverage
**Priority**: Low
**Status**: Open
**Description:** Limited integration tests
**Recommended Actions:**
- Implement comprehensive integration test suite

### 16. Test Infrastructure
**Priority**: Low
**Status**: Open
**Description:** Testing module could be more robust and easier to use  
**Recommended Actions:**
- Add test utilities for common CNI scenarios
- Create helper functions for network namespace testing
- Implement better test isolation and cleanup
- Add benchmarks for performance-critical paths

### 17. CI Integration
**Priority**: Low
**Status**: Open
**Description:** No continuous integration setup to run tests automatically
**Recommended Actions:**
- Set up GitHub Actions to run tests on each push

### 18. Insufficient Unit Tests
**Priority**: Low
**Status**: Open
**Description:** Critical functions lack unit tests
**Recommended Actions:**
- Identify key functions
- Write unit tests covering normal and edge cases
- Add property-based testing for IP generation
