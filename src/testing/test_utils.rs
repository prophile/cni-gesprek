use super::*;
use std::collections::HashMap;
use std::error::Error;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tempfile::{NamedTempFile, TempDir};

/// Utilities for common CNI testing scenarios
pub struct CniTestUtils;

impl CniTestUtils {
    /// Create a temporary CNI configuration file with given content
    pub fn create_temp_config(config_json: &str) -> Result<NamedTempFile, Box<dyn Error>> {
        let mut temp_file = NamedTempFile::new()?;
        temp_file.write_all(config_json.as_bytes())?;
        temp_file.flush()?;
        Ok(temp_file)
    }

    /// Create a basic ADD command configuration
    pub fn basic_add_config(name: &str, interface: &str, cidr: &str) -> String {
        format!(
            r#"{{
            "cniVersion": "1.0.0",
            "name": "{}",
            "type": "cni-gesprek",
            "master": "{}",
            "podCIDR": "{}"
        }}"#,
            name, interface, cidr
        )
    }

    /// Create a configuration with DNS settings
    pub fn config_with_dns(
        name: &str,
        interface: &str,
        cidr: &str,
        nameservers: &[&str],
        domain: Option<&str>,
    ) -> String {
        let dns_json = if let Some(domain) = domain {
            format!(
                r#"{{
                "nameservers": [{}],
                "domain": "{}"
            }}"#,
                nameservers
                    .iter()
                    .map(|ns| format!("\"{}\"", ns))
                    .collect::<Vec<_>>()
                    .join(","),
                domain
            )
        } else {
            format!(
                r#"{{
                "nameservers": [{}]
            }}"#,
                nameservers
                    .iter()
                    .map(|ns| format!("\"{}\"", ns))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        };

        format!(
            r#"{{
            "cniVersion": "1.0.0",
            "name": "{}",
            "type": "cni-gesprek",
            "master": "{}",
            "podCIDR": "{}",
            "dns": {}
        }}"#,
            name, interface, cidr, dns_json
        )
    }

    /// Generate standard CNI environment variables
    pub fn standard_cni_env(
        command: &str,
        container_id: &str,
        netns_path: &str,
        ifname: &str,
    ) -> HashMap<String, String> {
        let mut env = HashMap::new();
        env.insert("CNI_COMMAND".to_string(), command.to_string());
        env.insert("CNI_CONTAINERID".to_string(), container_id.to_string());
        env.insert("CNI_NETNS".to_string(), netns_path.to_string());
        env.insert("CNI_IFNAME".to_string(), ifname.to_string());
        env.insert("CNI_PATH".to_string(), "/opt/cni/bin".to_string());
        env
    }

    /// Validate JSON structure contains expected fields
    pub fn validate_json_structure(
        json_str: &str,
        expected_fields: &[&str],
    ) -> Result<serde_json::Value, String> {
        let json: serde_json::Value =
            serde_json::from_str(json_str).map_err(|e| format!("Invalid JSON: {}", e))?;

        for field in expected_fields {
            if !json.get(field).is_some() {
                return Err(format!("Missing required field: {}", field));
            }
        }

        Ok(json)
    }

    /// Extract IP address from CNI result JSON
    pub fn extract_ip_from_result(result_json: &str) -> Result<String, String> {
        let json = Self::validate_json_structure(result_json, &["ips"])?;

        let ips = json["ips"].as_array().ok_or("ips field is not an array")?;

        if ips.is_empty() {
            return Err("No IP addresses in result".to_string());
        }

        let first_ip = &ips[0];
        let address = first_ip["address"]
            .as_str()
            .ok_or("IP address field missing or not string")?;

        Ok(address.to_string())
    }

    /// Generate a unique test container ID
    pub fn unique_container_id(prefix: &str) -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        format!("{}-{}", prefix, timestamp)
    }

    /// Generate a unique network namespace path for testing
    pub fn unique_netns_path(prefix: &str) -> String {
        // Generate a pseudo-PID that looks realistic for testing
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            % 1000000; // Keep it as a reasonable PID range

        // Use a valid format that passes validation
        if prefix == "self" {
            "/proc/self/ns/net".to_string()
        } else {
            format!("/proc/{}/ns/net", timestamp)
        }
    }
}

/// Helper for network namespace testing scenarios
pub struct NetnsTestHelper {
    temp_dir: TempDir,
    netns_paths: Vec<PathBuf>,
}

impl NetnsTestHelper {
    /// Create a new network namespace test helper
    pub fn new() -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            temp_dir: TempDir::new()?,
            netns_paths: Vec::new(),
        })
    }

    /// Create a mock network namespace path for testing
    pub fn create_mock_netns(&mut self, name: &str) -> Result<PathBuf, Box<dyn Error>> {
        let netns_dir = self.temp_dir.path().join("netns");
        std::fs::create_dir_all(&netns_dir)?;

        let netns_path = netns_dir.join(name);
        std::fs::write(&netns_path, "")?; // Create empty file as placeholder

        self.netns_paths.push(netns_path.clone());
        Ok(netns_path)
    }

    /// Get all created network namespace paths
    pub fn get_netns_paths(&self) -> &[PathBuf] {
        &self.netns_paths
    }

    /// Clean up all created network namespaces
    pub fn cleanup(&mut self) {
        self.netns_paths.clear();
        // TempDir automatically cleans up when dropped
    }
}

/// Test isolation manager for ensuring tests don't interfere with each other
pub struct TestIsolationManager {
    active_tests: Mutex<HashMap<String, TestContext>>,
}

#[derive(Debug)]
struct TestContext {
    test_name: String,
    start_time: Instant,
    resources: Vec<String>,
}

impl TestIsolationManager {
    /// Create a new isolation manager
    pub fn new() -> Self {
        Self {
            active_tests: Mutex::new(HashMap::new()),
        }
    }

    /// Register a test for isolation tracking
    pub fn register_test(&self, test_name: &str) -> Result<(), String> {
        let mut tests = self.active_tests.lock().unwrap();

        if tests.contains_key(test_name) {
            return Err(format!("Test {} is already running", test_name));
        }

        tests.insert(
            test_name.to_string(),
            TestContext {
                test_name: test_name.to_string(),
                start_time: Instant::now(),
                resources: Vec::new(),
            },
        );

        Ok(())
    }

    /// Add a resource to track for a test
    pub fn add_resource(&self, test_name: &str, resource: &str) {
        if let Ok(mut tests) = self.active_tests.lock() {
            if let Some(context) = tests.get_mut(test_name) {
                context.resources.push(resource.to_string());
            }
        }
    }

    /// Unregister a test and clean up its resources
    pub fn unregister_test(&self, test_name: &str) -> Result<Duration, String> {
        let mut tests = self.active_tests.lock().unwrap();

        if let Some(context) = tests.remove(test_name) {
            let duration = context.start_time.elapsed();

            // Log cleanup of resources (in real implementation, this might do actual cleanup)
            for resource in context.resources {
                eprintln!("Cleaning up resource for {}: {}", test_name, resource);
            }

            Ok(duration)
        } else {
            Err(format!("Test {} was not registered", test_name))
        }
    }

    /// Get currently active tests
    pub fn get_active_tests(&self) -> Vec<String> {
        self.active_tests.lock().unwrap().keys().cloned().collect()
    }
}

/// Utility for command sequence validation
pub struct CommandSequenceValidator {
    expected_sequences: Vec<Vec<String>>,
}

impl CommandSequenceValidator {
    /// Create a new command sequence validator
    pub fn new() -> Self {
        Self {
            expected_sequences: Vec::new(),
        }
    }

    /// Add an expected command sequence
    pub fn expect_sequence(&mut self, commands: Vec<&str>) {
        self.expected_sequences
            .push(commands.iter().map(|s| s.to_string()).collect());
    }

    /// Validate that captured commands match expected sequences
    pub fn validate(&self, captured_commands: &[CapturedCommand]) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        for (i, expected_seq) in self.expected_sequences.iter().enumerate() {
            if !self.sequence_found_in_commands(expected_seq, captured_commands) {
                errors.push(format!(
                    "Expected command sequence {} not found: {:?}",
                    i + 1,
                    expected_seq
                ));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    fn sequence_found_in_commands(
        &self,
        sequence: &[String],
        commands: &[CapturedCommand],
    ) -> bool {
        for window in commands.windows(sequence.len()) {
            let matches = window.iter().zip(sequence.iter()).all(|(cmd, expected)| {
                format!("{} {}", cmd.program, cmd.args.join(" ")).contains(expected)
            });

            if matches {
                return true;
            }
        }
        false
    }
}

/// Performance testing utilities
pub struct PerformanceTestUtils;

impl PerformanceTestUtils {
    /// Time a function execution
    pub fn time_execution<F, R>(f: F) -> (R, Duration)
    where
        F: FnOnce() -> R,
    {
        let start = Instant::now();
        let result = f();
        let duration = start.elapsed();
        (result, duration)
    }

    /// Run a function multiple times and collect timing statistics
    pub fn benchmark<F, R>(f: F, iterations: usize) -> BenchmarkResult<R>
    where
        F: Fn() -> R,
    {
        let mut results = Vec::new();
        let mut timings = Vec::new();

        for _ in 0..iterations {
            let (result, timing) = Self::time_execution(&f);
            results.push(result);
            timings.push(timing);
        }

        BenchmarkResult {
            results,
            timings,
            iterations,
        }
    }
}

/// Result of a benchmark run
pub struct BenchmarkResult<R> {
    pub results: Vec<R>,
    pub timings: Vec<Duration>,
    pub iterations: usize,
}

impl<R> BenchmarkResult<R> {
    /// Get the average execution time
    pub fn average_time(&self) -> Duration {
        let total_nanos: u64 = self.timings.iter().map(|d| d.as_nanos() as u64).sum();
        Duration::from_nanos(total_nanos / self.iterations as u64)
    }

    /// Get the minimum execution time
    pub fn min_time(&self) -> Duration {
        *self
            .timings
            .iter()
            .min()
            .unwrap_or(&Duration::from_nanos(0))
    }

    /// Get the maximum execution time
    pub fn max_time(&self) -> Duration {
        *self
            .timings
            .iter()
            .max()
            .unwrap_or(&Duration::from_nanos(0))
    }

    /// Check if all executions completed within a time limit
    pub fn all_within_limit(&self, limit: Duration) -> bool {
        self.timings.iter().all(|&timing| timing <= limit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cni_test_utils_basic_config() {
        let config = CniTestUtils::basic_add_config("test", "eth0", "2001:db8::/64");
        assert!(config.contains("\"name\": \"test\""));
        assert!(config.contains("\"master\": \"eth0\""));
        assert!(config.contains("\"podCIDR\": \"2001:db8::/64\""));
    }

    #[test]
    fn test_unique_container_id() {
        let id1 = CniTestUtils::unique_container_id("test");
        let id2 = CniTestUtils::unique_container_id("test");
        assert_ne!(id1, id2);
        assert!(id1.starts_with("test-"));
    }

    #[test]
    fn test_json_validation() {
        let json = r#"{"cniVersion": "1.0.0", "interfaces": [], "ips": []}"#;
        let result = CniTestUtils::validate_json_structure(json, &["cniVersion", "interfaces"]);
        assert!(result.is_ok());

        let result = CniTestUtils::validate_json_structure(json, &["missing_field"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_netns_helper() {
        let mut helper = NetnsTestHelper::new().unwrap();
        let netns_path = helper.create_mock_netns("test-ns").unwrap();
        assert!(netns_path.exists());
        assert_eq!(helper.get_netns_paths().len(), 1);
    }

    #[test]
    fn test_isolation_manager() {
        let manager = TestIsolationManager::new();

        assert!(manager.register_test("test1").is_ok());
        assert!(manager.register_test("test1").is_err()); // Duplicate

        manager.add_resource("test1", "resource1");

        let duration = manager.unregister_test("test1").unwrap();
        assert!(duration.as_nanos() > 0);
    }

    #[test]
    fn test_command_sequence_validator() {
        let mut validator = CommandSequenceValidator::new();
        validator.expect_sequence(vec!["ip link add", "ip link set"]);

        let commands = vec![
            CapturedCommand {
                program: "ip".to_string(),
                args: vec!["link".to_string(), "add".to_string()],
                working_dir: None,
                description: "test".to_string(),
            },
            CapturedCommand {
                program: "ip".to_string(),
                args: vec!["link".to_string(), "set".to_string()],
                working_dir: None,
                description: "test".to_string(),
            },
        ];

        assert!(validator.validate(&commands).is_ok());
    }

    #[test]
    fn test_performance_utils() {
        let result = PerformanceTestUtils::benchmark(|| 42, 5);
        assert_eq!(result.iterations, 5);
        assert_eq!(result.results.len(), 5);
        assert!(result.results.iter().all(|&x| x == 42));

        let avg_time = result.average_time();
        assert!(avg_time.as_nanos() > 0);
    }
}
