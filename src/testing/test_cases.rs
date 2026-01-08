use crate::testing::{
    CniConfigBuilder, CniEnvBuilder, CommandValidators, TestCase, TestResult, TestRunner,
    TestScenarios,
};
use std::error::Error;

/// Test case for basic ADD command
pub struct BasicAddTest {
    pub name: String,
    pub config: CniConfigBuilder,
    pub env: CniEnvBuilder,
}

impl BasicAddTest {
    pub fn new() -> Self {
        let (config, env) = TestScenarios::basic_add();
        Self {
            name: "basic_add_test".to_string(),
            config,
            env,
        }
    }
}

impl Default for BasicAddTest {
    fn default() -> Self {
        Self::new()
    }
}

impl TestCase for BasicAddTest {
    fn name(&self) -> &str {
        &self.name
    }

    fn setup(&mut self) -> Result<(), Box<dyn Error>> {
        // No setup required for basic test
        Ok(())
    }

    fn execute(&mut self, runner: &TestRunner) -> Result<TestResult, Box<dyn Error>> {
        let mut result = runner.run_cni(&self.config, &self.env, true)?;
        result.captured_commands = runner.parse_dry_run_commands(&result.stderr);
        Ok(result)
    }

    fn validate(&self, result: &mut TestResult) {
        // Validate successful execution
        if result.exit_code != Some(0) {
            result.add_error("Expected exit code 0".to_string());
        }

        // Validate JSON output structure
        CommandValidators::expect_json_output(
            result,
            &["cniVersion", "interfaces", "ips"],
            "JSON output validation",
        );

        // Validate that interface checking was performed
        CommandValidators::expect_command(
            result,
            "ip",
            &["link", "show", "dev", "eth0"],
            "Interface existence check",
        );

        // Validate ipvlan creation
        CommandValidators::expect_ipvlan_creation(result, "eth0", "l2", "IPvlan creation");

        // Validate interface configuration in namespace
        let netns = "/proc/12345/ns/net";
        CommandValidators::expect_nsenter_command(
            result,
            netns,
            &["ip", "addr", "add"],
            "IP address assignment",
        );

        CommandValidators::expect_nsenter_command(
            result,
            netns,
            &["ip", "link", "set", "up", "dev", "eth0"],
            "Interface activation",
        );

        CommandValidators::expect_nsenter_command(
            result,
            netns,
            &["sysctl", "-w"],
            "accept_ra configuration",
        );
    }
}

/// Test case for ADD command with DNS configuration
pub struct AddWithDnsTest {
    pub name: String,
    pub config: CniConfigBuilder,
    pub env: CniEnvBuilder,
}

impl AddWithDnsTest {
    pub fn new() -> Self {
        let (config, env) = TestScenarios::add_with_dns();
        Self {
            name: "add_with_dns_test".to_string(),
            config,
            env,
        }
    }
}

impl Default for AddWithDnsTest {
    fn default() -> Self {
        Self::new()
    }
}

impl TestCase for AddWithDnsTest {
    fn name(&self) -> &str {
        &self.name
    }

    fn setup(&mut self) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    fn execute(&mut self, runner: &TestRunner) -> Result<TestResult, Box<dyn Error>> {
        let mut result = runner.run_cni(&self.config, &self.env, true)?;
        result.captured_commands = runner.parse_dry_run_commands(&result.stderr);
        Ok(result)
    }

    fn validate(&self, result: &mut TestResult) {
        // Basic validation
        if result.exit_code != Some(0) {
            result.add_error("Expected exit code 0".to_string());
        }

        // Validate JSON output includes DNS
        CommandValidators::expect_json_output(
            result,
            &["cniVersion", "interfaces", "ips", "dns"],
            "JSON output with DNS validation",
        );

        // Validate interface checking for eth1
        CommandValidators::expect_command(
            result,
            "ip",
            &["link", "show", "dev", "eth1"],
            "Interface existence check",
        );
    }
}

/// Test case for interface auto-detection
pub struct AutoDetectTest {
    pub name: String,
    pub config: CniConfigBuilder,
    pub env: CniEnvBuilder,
}

impl AutoDetectTest {
    pub fn new() -> Self {
        let (config, env) = TestScenarios::add_auto_detect();
        Self {
            name: "auto_detect_test".to_string(),
            config,
            env,
        }
    }
}

impl Default for AutoDetectTest {
    fn default() -> Self {
        Self::new()
    }
}

impl TestCase for AutoDetectTest {
    fn name(&self) -> &str {
        &self.name
    }

    fn setup(&mut self) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    fn execute(&mut self, runner: &TestRunner) -> Result<TestResult, Box<dyn Error>> {
        let mut result = runner.run_cni(&self.config, &self.env, true)?;
        result.captured_commands = runner.parse_dry_run_commands(&result.stderr);
        Ok(result)
    }

    fn validate(&self, result: &mut TestResult) {
        // Validate that interface detection was performed
        CommandValidators::expect_command(
            result,
            "ip",
            &["-6", "-j", "route", "show", "default"],
            "IPv6 default route detection",
        );

        // The detected interface should be used for subsequent operations
        // This will depend on the mock response from the dry run driver
    }
}

/// Test case for subnet detection
pub struct SubnetDetectTest {
    pub name: String,
    pub config: CniConfigBuilder,
    pub env: CniEnvBuilder,
}

impl SubnetDetectTest {
    pub fn new() -> Self {
        let (config, env) = TestScenarios::add_detect_subnet();
        Self {
            name: "subnet_detect_test".to_string(),
            config,
            env,
        }
    }
}

impl Default for SubnetDetectTest {
    fn default() -> Self {
        Self::new()
    }
}

impl TestCase for SubnetDetectTest {
    fn name(&self) -> &str {
        &self.name
    }

    fn setup(&mut self) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    fn execute(&mut self, runner: &TestRunner) -> Result<TestResult, Box<dyn Error>> {
        let mut result = runner.run_cni(&self.config, &self.env, true)?;
        result.captured_commands = runner.parse_dry_run_commands(&result.stderr);
        Ok(result)
    }

    fn validate(&self, result: &mut TestResult) {
        // Validate that subnet detection was performed
        CommandValidators::expect_command(
            result,
            "ip",
            &["-j", "-6", "addr", "show", "dev", "eth2", "scope", "global"],
            "Subnet detection on interface",
        );

        // Validate gateway detection
        CommandValidators::expect_command(
            result,
            "ip",
            &["-6", "-j", "route", "show", "default", "dev", "eth2"],
            "Gateway detection on interface",
        );
    }
}

/// Test case for VERSION command
pub struct VersionTest {
    pub name: String,
    pub config: CniConfigBuilder,
    pub env: CniEnvBuilder,
}

impl VersionTest {
    pub fn new() -> Self {
        let (config, env) = TestScenarios::version();
        Self {
            name: "version_test".to_string(),
            config,
            env,
        }
    }
}

impl Default for VersionTest {
    fn default() -> Self {
        Self::new()
    }
}

impl TestCase for VersionTest {
    fn name(&self) -> &str {
        &self.name
    }

    fn setup(&mut self) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    fn execute(&mut self, runner: &TestRunner) -> Result<TestResult, Box<dyn Error>> {
        runner.run_cni(&self.config, &self.env, false)
    }

    fn validate(&self, result: &mut TestResult) {
        if result.exit_code != Some(0) {
            result.add_error("Expected exit code 0".to_string());
        }

        // Validate VERSION response
        CommandValidators::expect_json_output(
            result,
            &["cniVersion", "supportedVersions"],
            "VERSION response validation",
        );

        // Check for specific version content
        if !result.stdout.contains("1.0.0") {
            result.add_error("Expected version 1.0.0 in output".to_string());
        }
    }
}

/// Test case for DEL command
pub struct DeleteTest {
    pub name: String,
    pub config: CniConfigBuilder,
    pub env: CniEnvBuilder,
}

impl DeleteTest {
    pub fn new() -> Self {
        let (config, env) = TestScenarios::delete();
        Self {
            name: "delete_test".to_string(),
            config,
            env,
        }
    }
}

impl Default for DeleteTest {
    fn default() -> Self {
        Self::new()
    }
}

impl TestCase for DeleteTest {
    fn name(&self) -> &str {
        &self.name
    }

    fn setup(&mut self) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    fn execute(&mut self, runner: &TestRunner) -> Result<TestResult, Box<dyn Error>> {
        runner.run_cni(&self.config, &self.env, false)
    }

    fn validate(&self, result: &mut TestResult) {
        if result.exit_code != Some(0) {
            result.add_error("Expected exit code 0".to_string());
        }

        // DEL should return success JSON
        CommandValidators::expect_json_output(result, &["cniVersion"], "DEL response validation");
    }
}

/// Test case for CHECK command
pub struct CheckTest {
    pub name: String,
    pub config: CniConfigBuilder,
    pub env: CniEnvBuilder,
}

impl CheckTest {
    pub fn new() -> Self {
        let (config, env) = TestScenarios::check();
        Self {
            name: "check_test".to_string(),
            config,
            env,
        }
    }
}

impl Default for CheckTest {
    fn default() -> Self {
        Self::new()
    }
}

impl TestCase for CheckTest {
    fn name(&self) -> &str {
        &self.name
    }

    fn setup(&mut self) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    fn execute(&mut self, runner: &TestRunner) -> Result<TestResult, Box<dyn Error>> {
        runner.run_cni(&self.config, &self.env, false)
    }

    fn validate(&self, result: &mut TestResult) {
        if result.exit_code != Some(0) {
            result.add_error("Expected exit code 0".to_string());
        }

        // CHECK should return success JSON
        CommandValidators::expect_json_output(result, &["cniVersion"], "CHECK response validation");
    }
}

/// Test case for STATUS command
pub struct StatusTest {
    pub name: String,
    pub config: CniConfigBuilder,
    pub env: CniEnvBuilder,
}

impl StatusTest {
    pub fn new() -> Self {
        let (config, env) = TestScenarios::status();
        Self {
            name: "status_test".to_string(),
            config,
            env,
        }
    }
}

impl Default for StatusTest {
    fn default() -> Self {
        Self::new()
    }
}

impl TestCase for StatusTest {
    fn name(&self) -> &str {
        &self.name
    }

    fn setup(&mut self) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    fn execute(&mut self, runner: &TestRunner) -> Result<TestResult, Box<dyn Error>> {
        let mut result = runner.run_cni(&self.config, &self.env, true)?;
        result.captured_commands = runner.parse_dry_run_commands(&result.stderr);
        Ok(result)
    }

    fn validate(&self, result: &mut TestResult) {
        if result.exit_code != Some(0) {
            result.add_error("Expected exit code 0".to_string());
        }

        // STATUS should check interface
        CommandValidators::expect_command(
            result,
            "ip",
            &["link", "show", "dev", "eth0"],
            "STATUS interface check",
        );
    }
}

/// Convenience function to get all standard test cases
pub fn get_all_test_cases() -> Vec<Box<dyn TestCase>> {
    vec![
        Box::new(BasicAddTest::new()),
        Box::new(AddWithDnsTest::new()),
        Box::new(AutoDetectTest::new()),
        Box::new(SubnetDetectTest::new()),
        Box::new(VersionTest::new()),
        Box::new(DeleteTest::new()),
        Box::new(CheckTest::new()),
        Box::new(StatusTest::new()),
    ]
}
