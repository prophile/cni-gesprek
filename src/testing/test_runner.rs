use crate::testing::{CapturedCommand, CniConfigBuilder, CniEnvBuilder, CommandCaptureDriver};
use serde_json::Value;
use std::error::Error;
use std::fmt;
use std::io::Write;
use std::process::{Command, Stdio};

/// Result of a test execution
#[derive(Debug, Clone)]
pub struct TestResult {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub captured_commands: Vec<CapturedCommand>,
    pub errors: Vec<String>,
}

impl TestResult {
    pub fn is_success(&self) -> bool {
        self.success && self.errors.is_empty()
    }

    pub fn add_error(&mut self, error: String) {
        self.errors.push(error);
        self.success = false;
    }
}

impl fmt::Display for TestResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "Test Result: {}",
            if self.success { "PASS" } else { "FAIL" }
        )?;
        if !self.stdout.is_empty() {
            writeln!(f, "STDOUT:\n{}", self.stdout)?;
        }
        if !self.stderr.is_empty() {
            writeln!(f, "STDERR:\n{}", self.stderr)?;
        }
        if !self.errors.is_empty() {
            writeln!(f, "VALIDATION ERRORS:")?;
            for error in &self.errors {
                writeln!(f, "  - {}", error)?;
            }
        }
        if !self.captured_commands.is_empty() {
            writeln!(f, "CAPTURED COMMANDS:")?;
            for (i, cmd) in self.captured_commands.iter().enumerate() {
                writeln!(f, "  {}. {} {}", i + 1, cmd.program, cmd.args.join(" "))?;
                if !cmd.description.is_empty() {
                    writeln!(f, "     ({})", cmd.description)?;
                }
            }
        }
        Ok(())
    }
}

/// A test case definition
pub trait TestCase {
    fn name(&self) -> &str;
    fn setup(&mut self) -> Result<(), Box<dyn Error>>;
    fn execute(&mut self, runner: &TestRunner) -> Result<TestResult, Box<dyn Error>>;
    fn validate(&self, result: &mut TestResult);
}

/// Test runner for CNI plugin testing
pub struct TestRunner {
    pub binary_path: String,
    pub capture_driver: CommandCaptureDriver,
}

impl TestRunner {
    pub fn new(binary_path: &str) -> Self {
        Self {
            binary_path: binary_path.to_string(),
            capture_driver: CommandCaptureDriver::new(),
        }
    }

    /// Run the CNI plugin with given configuration and environment
    pub fn run_cni(
        &self,
        config: &CniConfigBuilder,
        env: &CniEnvBuilder,
        dry_run: bool,
    ) -> Result<TestResult, Box<dyn Error>> {
        let config_json = config.clone().build_json_string();
        let env_vars = env.clone().build();

        let mut cmd = Command::new(&self.binary_path);

        // Set environment variables
        for (key, value) in env_vars.iter() {
            cmd.env(key, value);
        }

        // Add dry run flag if requested
        if dry_run {
            cmd.arg("--dry-run");
        }

        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd.spawn()?;

        // Write config to stdin
        if let Some(stdin) = child.stdin.as_mut() {
            stdin.write_all(config_json.as_bytes())?;
        }

        let output = child.wait_with_output()?;

        Ok(TestResult {
            success: output.status.success(),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            exit_code: output.status.code(),
            captured_commands: Vec::new(), // Will be filled by validation logic for dry runs
            errors: Vec::new(),
        })
    }

    /// Parse captured commands from dry run output
    pub fn parse_dry_run_commands(&self, stderr: &str) -> Vec<CapturedCommand> {
        let mut commands = Vec::new();

        for line in stderr.lines() {
            if let Some(command_str) = line.strip_prefix("[DRY-RUN] ") {
                // Simple parsing - in a real implementation, you might want more sophisticated parsing
                let parts: Vec<&str> = command_str.split_whitespace().collect();
                if !parts.is_empty() {
                    commands.push(CapturedCommand {
                        program: parts[0].to_string(),
                        args: parts[1..].iter().map(|s| s.to_string()).collect(),
                        working_dir: None,
                        description: command_str.to_string(),
                    });
                }
            }
        }

        commands
    }

    /// Run a test case
    pub fn run_test(&self, mut test_case: Box<dyn TestCase>) -> Result<TestResult, Box<dyn Error>> {
        test_case.setup()?;
        let mut result = test_case.execute(self)?;
        test_case.validate(&mut result);
        Ok(result)
    }

    /// Run multiple test cases
    pub fn run_tests(
        &self,
        test_cases: Vec<Box<dyn TestCase>>,
    ) -> Vec<(String, Result<TestResult, Box<dyn Error>>)> {
        test_cases
            .into_iter()
            .map(|test_case| {
                let name = test_case.name().to_string();
                let result = self.run_test(test_case);
                (name, result)
            })
            .collect()
    }
}

/// Helper functions for common command validations
pub struct CommandValidators;

impl CommandValidators {
    /// Validate that a specific command was executed
    pub fn expect_command(
        result: &mut TestResult,
        program: &str,
        args: &[&str],
        description: &str,
    ) {
        let found = result.captured_commands.iter().any(|cmd| {
            cmd.program == program
                && cmd.args == args.iter().map(|s| s.to_string()).collect::<Vec<_>>()
        });

        if !found {
            result.add_error(format!(
                "{}: Expected command '{}' with args {:?} not found",
                description, program, args
            ));
        }
    }

    /// Validate that commands appear in the correct order
    pub fn expect_command_order(
        result: &mut TestResult,
        expected_sequence: &[(String, Vec<String>)],
        description: &str,
    ) {
        let mut last_found_index = 0;

        for (expected_program, expected_args) in expected_sequence {
            let found = result.captured_commands[last_found_index..]
                .iter()
                .enumerate()
                .find(|(_, cmd)| cmd.program == *expected_program && cmd.args == *expected_args)
                .map(|(i, _)| i + last_found_index);

            match found {
                Some(index) => {
                    last_found_index = index + 1;
                }
                None => {
                    result.add_error(format!(
                        "{}: Expected command '{}' with args {:?} not found in correct order",
                        description, expected_program, expected_args
                    ));
                    return;
                }
            }
        }
    }

    /// Validate that nsenter commands are properly formatted
    pub fn expect_nsenter_command(
        result: &mut TestResult,
        netns: &str,
        inner_command: &[&str],
        description: &str,
    ) {
        let expected_args: Vec<String> = std::iter::once(format!("--net={}", netns))
            .chain(std::iter::once("-F".to_string()))
            .chain(std::iter::once("--".to_string()))
            .chain(inner_command.iter().map(|s| s.to_string()))
            .collect();

        let found = result
            .captured_commands
            .iter()
            .any(|cmd| cmd.program == "nsenter" && cmd.args == expected_args);

        if !found {
            result.add_error(format!(
                "{}: Expected nsenter command with netns '{}' and inner command {:?} not found",
                description, netns, inner_command
            ));
        }
    }

    /// Validate JSON output structure
    pub fn expect_json_output(
        result: &mut TestResult,
        expected_fields: &[&str],
        description: &str,
    ) {
        match serde_json::from_str::<Value>(&result.stdout) {
            Ok(json) => {
                for field in expected_fields {
                    if !json.get(field).is_some() {
                        result.add_error(format!(
                            "{}: Missing expected JSON field '{}'",
                            description, field
                        ));
                    }
                }
            }
            Err(e) => {
                result.add_error(format!("{}: Invalid JSON output: {}", description, e));
            }
        }
    }

    /// Validate that ipvlan creation uses correct parameters
    pub fn expect_ipvlan_creation(
        result: &mut TestResult,
        parent: &str,
        mode: &str,
        description: &str,
    ) {
        let found = result.captured_commands.iter().any(|cmd| {
            cmd.program == "ip"
                && cmd.args.contains(&"add".to_string())
                && cmd.args.contains(&"type".to_string())
                && cmd.args.contains(&"ipvlan".to_string())
                && cmd.args.contains(&format!("mode").to_string())
                && cmd.args.contains(&mode.to_string())
                && cmd.args.contains(&"link".to_string())
                && cmd.args.contains(&parent.to_string())
        });

        if !found {
            result.add_error(format!(
                "{}: Expected ipvlan creation with parent '{}' and mode '{}' not found",
                description, parent, mode
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_validators() {
        let mut result = TestResult {
            success: true,
            stdout: String::new(),
            stderr: String::new(),
            exit_code: Some(0),
            captured_commands: vec![CapturedCommand {
                program: "ip".to_string(),
                args: vec!["link", "show", "eth0"]
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
                working_dir: None,
                description: "test".to_string(),
            }],
            errors: Vec::new(),
        };

        CommandValidators::expect_command(&mut result, "ip", &["link", "show", "eth0"], "test");
        assert!(result.errors.is_empty());

        CommandValidators::expect_command(&mut result, "ip", &["link", "show", "eth1"], "test");
        assert!(!result.errors.is_empty());
    }
}
