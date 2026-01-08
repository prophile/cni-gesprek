use crate::error::{CniResult, EnvironmentError};
use regex::Regex;
use std::collections::HashMap;
use std::env;

/// Trait for abstracting environment variable access
pub trait EnvironmentProvider {
    /// Get an environment variable value
    fn get(&self, key: &str) -> Option<String>;

    /// Get an environment variable, returning an error if not set
    fn get_required(&self, key: &str) -> CniResult<String> {
        self.get(key).ok_or_else(|| {
            EnvironmentError::MissingVariable {
                variable: key.to_string(),
                command: "unknown".to_string(),
            }
            .into()
        })
    }

    /// Check if an environment variable exists (regardless of value)
    fn exists(&self, key: &str) -> bool {
        self.get(key).is_some()
    }

    /// Get multiple required environment variables at once
    fn get_required_multiple(&self, keys: &[&str]) -> CniResult<HashMap<String, String>> {
        let mut result = HashMap::new();
        for key in keys {
            result.insert(key.to_string(), self.get_required(key)?);
        }
        Ok(result)
    }
}

/// Real implementation that reads from system environment
#[derive(Debug, Default)]
pub struct SystemEnvironmentProvider;

impl EnvironmentProvider for SystemEnvironmentProvider {
    fn get(&self, key: &str) -> Option<String> {
        env::var(key).ok()
    }
}

/// Mock implementation for testing
#[derive(Debug, Default)]
pub struct MockEnvironmentProvider {
    variables: HashMap<String, String>,
}

impl MockEnvironmentProvider {
    /// Create a new mock environment provider
    pub fn new() -> Self {
        Self {
            variables: HashMap::new(),
        }
    }

    /// Set an environment variable for testing
    pub fn set<K, V>(&mut self, key: K, value: V) -> &mut Self
    where
        K: Into<String>,
        V: Into<String>,
    {
        self.variables.insert(key.into(), value.into());
        self
    }

    /// Set multiple environment variables at once
    pub fn set_multiple<I, K, V>(&mut self, vars: I) -> &mut Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        for (key, value) in vars {
            self.variables.insert(key.into(), value.into());
        }
        self
    }

    /// Remove an environment variable
    pub fn unset(&mut self, key: &str) -> &mut Self {
        self.variables.remove(key);
        self
    }

    /// Clear all environment variables
    pub fn clear(&mut self) -> &mut Self {
        self.variables.clear();
        self
    }
}

impl EnvironmentProvider for MockEnvironmentProvider {
    fn get(&self, key: &str) -> Option<String> {
        self.variables.get(key).cloned()
    }
}

/// CNI-specific environment variables container
#[derive(Debug, Clone)]
pub struct CniEnvironment {
    pub command: String,
    pub netns: Option<String>,
    pub ifname: Option<String>,
    pub container_id: Option<String>,
    pub path: Option<String>,
}

impl CniEnvironment {
    /// Load CNI environment from an environment provider
    pub fn load<E: EnvironmentProvider>(env: &E) -> Self {
        Self {
            command: env
                .get("CNI_COMMAND")
                .unwrap_or_else(|| "VERSION".to_string()),
            netns: env.get("CNI_NETNS"),
            ifname: env.get("CNI_IFNAME"),
            container_id: env.get("CNI_CONTAINERID"),
            path: env.get("CNI_PATH"),
        }
    }

    /// Validate that all required environment variables are present and have valid values for a given command
    pub fn validate_for_command(&self) -> CniResult<()> {
        match self.command.as_str() {
            "ADD" | "DEL" | "CHECK" => {
                // Check presence of required variables
                if self.netns.is_none() {
                    return Err(EnvironmentError::MissingVariable {
                        variable: "CNI_NETNS".to_string(),
                        command: self.command.clone(),
                    }
                    .into());
                }
                if self.ifname.is_none() {
                    return Err(EnvironmentError::MissingVariable {
                        variable: "CNI_IFNAME".to_string(),
                        command: self.command.clone(),
                    }
                    .into());
                }
                if self.container_id.is_none() && (self.command == "DEL" || self.command == "CHECK")
                {
                    return Err(EnvironmentError::MissingVariable {
                        variable: "CNI_CONTAINERID".to_string(),
                        command: self.command.clone(),
                    }
                    .into());
                }

                // Validate content/format of required variables
                if let Some(ref netns) = self.netns {
                    self.validate_netns_format(netns)?;
                }

                if let Some(ref ifname) = self.ifname {
                    self.validate_ifname_format(ifname)?;
                }

                if let Some(ref container_id) = self.container_id {
                    self.validate_container_id_format(container_id)?;
                }
            }
            "VERSION" | "STATUS" | "GC" => {
                // These commands don't require additional environment variables
            }
            _ => {
                return Err(EnvironmentError::InvalidValue {
                    name: "CNI_COMMAND".to_string(),
                    value: self.command.clone(),
                    expected: "ADD, DEL, CHECK, VERSION, STATUS, or GC".to_string(),
                }
                .into());
            }
        }
        Ok(())
    }

    /// Get the network namespace path, returning an error if not set
    pub fn get_netns(&self) -> CniResult<&str> {
        self.netns.as_deref().ok_or_else(|| {
            EnvironmentError::MissingVariable {
                variable: "CNI_NETNS".to_string(),
                command: "unknown".to_string(),
            }
            .into()
        })
    }

    /// Get the interface name, returning an error if not set
    pub fn get_ifname(&self) -> CniResult<&str> {
        self.ifname.as_deref().ok_or_else(|| {
            EnvironmentError::MissingVariable {
                variable: "CNI_IFNAME".to_string(),
                command: "unknown".to_string(),
            }
            .into()
        })
    }

    /// Get the container ID, returning an error if not set
    pub fn get_container_id(&self) -> CniResult<&str> {
        self.container_id.as_deref().ok_or_else(|| {
            EnvironmentError::MissingVariable {
                variable: "CNI_CONTAINERID".to_string(),
                command: "unknown".to_string(),
            }
            .into()
        })
    }

    /// Validate CNI_NETNS path format
    fn validate_netns_format(&self, netns: &str) -> CniResult<()> {
        // Validate that the path looks like a valid network namespace path

        // Check for common netns path patterns
        let netns_patterns = [
            r"^/proc/\d+/ns/net$",              // /proc/{pid}/ns/net
            r"^/proc/self/ns/net$",             // /proc/self/ns/net
            r"^/var/run/netns/[a-zA-Z0-9_-]+$", // /var/run/netns/{name}
            r"^/run/netns/[a-zA-Z0-9_-]+$",     // /run/netns/{name}
        ];

        let is_valid = netns_patterns.iter().any(|pattern| {
            Regex::new(pattern)
                .map(|re| re.is_match(netns))
                .unwrap_or(false)
        });

        if !is_valid {
            return Err(EnvironmentError::InvalidValue {
                name: "CNI_NETNS".to_string(),
                value: netns.to_string(),
                expected: "valid network namespace path (e.g., /proc/{pid}/ns/net or /var/run/netns/{name})".to_string(),
            }.into());
        }

        Ok(())
    }

    /// Validate CNI_IFNAME format
    fn validate_ifname_format(&self, ifname: &str) -> CniResult<()> {
        // Linux network interface naming rules
        // - Maximum 15 characters (IFNAMSIZ - 1)
        // - Can contain alphanumeric characters, hyphens, underscores, dots
        // - Cannot start with a dot
        // - Cannot be empty

        if ifname.is_empty() {
            return Err(EnvironmentError::InvalidValue {
                name: "CNI_IFNAME".to_string(),
                value: ifname.to_string(),
                expected: "non-empty interface name".to_string(),
            }
            .into());
        }

        if ifname.len() > 15 {
            return Err(EnvironmentError::InvalidValue {
                name: "CNI_IFNAME".to_string(),
                value: ifname.to_string(),
                expected: "interface name with maximum 15 characters".to_string(),
            }
            .into());
        }

        if ifname.starts_with('.') {
            return Err(EnvironmentError::InvalidValue {
                name: "CNI_IFNAME".to_string(),
                value: ifname.to_string(),
                expected: "interface name that does not start with a dot".to_string(),
            }
            .into());
        }

        // Check for valid characters (alphanumeric, hyphen, underscore, dot)
        let valid_chars_re = Regex::new(r"^[a-zA-Z0-9._-]+$").unwrap();
        if !valid_chars_re.is_match(ifname) {
            return Err(EnvironmentError::InvalidValue {
                name: "CNI_IFNAME".to_string(),
                value: ifname.to_string(),
                expected: "interface name containing only alphanumeric characters, hyphens, underscores, and dots".to_string(),
            }.into());
        }

        Ok(())
    }

    /// Validate CNI_CONTAINERID format  
    fn validate_container_id_format(&self, container_id: &str) -> CniResult<()> {
        // Container ID validation rules:
        // - Should not be empty
        // - Should contain only alphanumeric characters, hyphens, underscores
        // - Reasonable length limits (between 1 and 64 characters)

        if container_id.is_empty() {
            return Err(EnvironmentError::InvalidValue {
                name: "CNI_CONTAINERID".to_string(),
                value: container_id.to_string(),
                expected: "non-empty container ID".to_string(),
            }
            .into());
        }

        if container_id.len() > 64 {
            return Err(EnvironmentError::InvalidValue {
                name: "CNI_CONTAINERID".to_string(),
                value: container_id.to_string(),
                expected: "container ID with maximum 64 characters".to_string(),
            }
            .into());
        }

        // Check for valid characters (alphanumeric, hyphen, underscore)
        let valid_chars_re = Regex::new(r"^[a-zA-Z0-9_-]+$").unwrap();
        if !valid_chars_re.is_match(container_id) {
            return Err(EnvironmentError::InvalidValue {
                name: "CNI_CONTAINERID".to_string(),
                value: container_id.to_string(),
                expected:
                    "container ID containing only alphanumeric characters, hyphens, and underscores"
                        .to_string(),
            }
            .into());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_environment_provider() {
        let provider = SystemEnvironmentProvider::default();

        // Test getting an environment variable that should exist
        let path = provider.get("PATH");
        assert!(path.is_some());

        // Test getting a non-existent variable
        let nonexistent = provider.get("NONEXISTENT_VAR_12345");
        assert!(nonexistent.is_none());
    }

    #[test]
    fn test_mock_environment_provider() {
        let mut provider = MockEnvironmentProvider::new();

        // Initially empty
        assert!(provider.get("TEST_VAR").is_none());

        // Set a variable
        provider.set("TEST_VAR", "test_value");
        assert_eq!(provider.get("TEST_VAR"), Some("test_value".to_string()));

        // Test required variable that exists
        assert!(provider.get_required("TEST_VAR").is_ok());

        // Test required variable that doesn't exist
        assert!(provider.get_required("MISSING_VAR").is_err());

        // Test multiple variables
        provider.set_multiple([("VAR1", "value1"), ("VAR2", "value2")]);

        let result = provider.get_required_multiple(&["TEST_VAR", "VAR1", "VAR2"]);
        assert!(result.is_ok());
        let vars = result.unwrap();
        assert_eq!(vars.len(), 3);
        assert_eq!(vars["TEST_VAR"], "test_value");
        assert_eq!(vars["VAR1"], "value1");
        assert_eq!(vars["VAR2"], "value2");
    }

    #[test]
    fn test_cni_environment_load() {
        let mut provider = MockEnvironmentProvider::new();
        provider
            .set("CNI_COMMAND", "ADD")
            .set("CNI_NETNS", "/proc/123/ns/net")
            .set("CNI_IFNAME", "eth0")
            .set("CNI_CONTAINERID", "container123")
            .set("CNI_PATH", "/usr/lib/cni");

        let cni_env = CniEnvironment::load(&provider);

        assert_eq!(cni_env.command, "ADD");
        assert_eq!(cni_env.netns, Some("/proc/123/ns/net".to_string()));
        assert_eq!(cni_env.ifname, Some("eth0".to_string()));
        assert_eq!(cni_env.container_id, Some("container123".to_string()));
        assert_eq!(cni_env.path, Some("/usr/lib/cni".to_string()));
    }

    #[test]
    fn test_cni_environment_validation() {
        let mut provider = MockEnvironmentProvider::new();

        // Test ADD command validation - should require netns, ifname
        provider
            .set("CNI_COMMAND", "ADD")
            .set("CNI_NETNS", "/proc/123/ns/net")
            .set("CNI_IFNAME", "eth0");

        let cni_env = CniEnvironment::load(&provider);
        assert!(cni_env.validate_for_command().is_ok());

        // Test DEL command validation - should require container_id too
        provider.set("CNI_COMMAND", "DEL");
        let cni_env = CniEnvironment::load(&provider);
        assert!(cni_env.validate_for_command().is_err()); // Missing container_id

        provider.set("CNI_CONTAINERID", "container123");
        let cni_env = CniEnvironment::load(&provider);
        assert!(cni_env.validate_for_command().is_ok());

        // Test VERSION command - should not require additional vars
        provider.clear().set("CNI_COMMAND", "VERSION");
        let cni_env = CniEnvironment::load(&provider);
        assert!(cni_env.validate_for_command().is_ok());
    }

    #[test]
    fn test_cni_environment_getters() {
        let mut provider = MockEnvironmentProvider::new();
        provider
            .set("CNI_COMMAND", "ADD")
            .set("CNI_NETNS", "/proc/123/ns/net")
            .set("CNI_IFNAME", "eth0")
            .set("CNI_CONTAINERID", "container123");

        let cni_env = CniEnvironment::load(&provider);

        assert_eq!(cni_env.get_netns().unwrap(), "/proc/123/ns/net");
        assert_eq!(cni_env.get_ifname().unwrap(), "eth0");
        assert_eq!(cni_env.get_container_id().unwrap(), "container123");

        // Test error cases
        let empty_env = CniEnvironment::load(&MockEnvironmentProvider::new());
        assert!(empty_env.get_netns().is_err());
        assert!(empty_env.get_ifname().is_err());
        assert!(empty_env.get_container_id().is_err());
    }

    #[test]
    fn test_cni_netns_validation() {
        let mut provider = MockEnvironmentProvider::new();
        provider.set("CNI_COMMAND", "ADD").set("CNI_IFNAME", "eth0");

        // Test valid netns paths
        let valid_paths = [
            "/proc/123/ns/net",
            "/proc/9999/ns/net",
            "/proc/self/ns/net", // Special case for current process
            "/var/run/netns/test",
            "/var/run/netns/test-namespace",
            "/var/run/netns/test_namespace",
            "/run/netns/myns",
            "/run/netns/my-test-ns",
        ];

        for path in &valid_paths {
            provider.set("CNI_NETNS", *path);
            let cni_env = CniEnvironment::load(&provider);
            assert!(
                cni_env.validate_for_command().is_ok(),
                "Valid netns path should pass validation: {}",
                path
            );
        }

        // Test invalid netns paths
        let invalid_paths = [
            "",                            // Empty
            "/invalid/path",               // Not a netns path
            "/proc/abc/ns/net",            // Non-numeric pid
            "/proc/123/ns/invalid",        // Wrong ns type
            "proc/123/ns/net",             // Missing leading slash
            "/var/run/netns/",             // Missing name
            "/var/run/netns/test space",   // Space in name
            "/var/run/netns/test@invalid", // Invalid character
        ];

        for path in &invalid_paths {
            provider.set("CNI_NETNS", *path);
            let cni_env = CniEnvironment::load(&provider);
            assert!(
                cni_env.validate_for_command().is_err(),
                "Invalid netns path should fail validation: {}",
                path
            );
        }
    }

    #[test]
    fn test_cni_ifname_validation() {
        let mut provider = MockEnvironmentProvider::new();
        provider
            .set("CNI_COMMAND", "ADD")
            .set("CNI_NETNS", "/proc/123/ns/net");

        // Test valid interface names
        let valid_names = [
            "eth0",
            "eth1",
            "veth1234",
            "cni-12345",
            "test_interface",
            "lo",
            "test.sub",
            "a",               // Minimum length
            "123456789012345", // Maximum length (15 chars)
        ];

        for name in &valid_names {
            provider.set("CNI_IFNAME", *name);
            let cni_env = CniEnvironment::load(&provider);
            assert!(
                cni_env.validate_for_command().is_ok(),
                "Valid interface name should pass validation: {}",
                name
            );
        }

        // Test invalid interface names
        let invalid_names = [
            "",                 // Empty
            "1234567890123456", // Too long (16+ chars)
            ".eth0",            // Starts with dot
            "eth 0",            // Space
            "eth@0",            // Invalid character
            "eth#0",            // Invalid character
            "eth/0",            // Invalid character
            "eth\\0",           // Invalid character
            "eth:0",            // Invalid character
        ];

        for name in &invalid_names {
            provider.set("CNI_IFNAME", *name);
            let cni_env = CniEnvironment::load(&provider);
            assert!(
                cni_env.validate_for_command().is_err(),
                "Invalid interface name should fail validation: {}",
                name
            );
        }
    }

    #[test]
    fn test_cni_containerid_validation() {
        let mut provider = MockEnvironmentProvider::new();
        provider
            .set("CNI_COMMAND", "DEL")
            .set("CNI_NETNS", "/proc/123/ns/net")
            .set("CNI_IFNAME", "eth0");

        // Test valid container IDs
        let valid_ids = [
            "container123",
            "test-container",
            "test_container",
            "a",                                                                // Minimum length
            "ABCD1234",                                                         // Uppercase
            "test-123_456", // Mix of valid chars
            "1234567890123456789012345678901234567890123456789012345678901234", // 64 chars (max)
        ];

        for id in &valid_ids {
            provider.set("CNI_CONTAINERID", *id);
            let cni_env = CniEnvironment::load(&provider);
            assert!(
                cni_env.validate_for_command().is_ok(),
                "Valid container ID should pass validation: {}",
                id
            );
        }

        // Test invalid container IDs
        let invalid_ids = [
            "",                                                                  // Empty
            "12345678901234567890123456789012345678901234567890123456789012345", // 65 chars (too long)
            "test container",                                                    // Space
            "test@container",  // Invalid character
            "test.container",  // Invalid character
            "test/container",  // Invalid character
            "test\\container", // Invalid character
            "test:container",  // Invalid character
            "test#container",  // Invalid character
        ];

        for id in &invalid_ids {
            provider.set("CNI_CONTAINERID", *id);
            let cni_env = CniEnvironment::load(&provider);
            assert!(
                cni_env.validate_for_command().is_err(),
                "Invalid container ID should fail validation: {}",
                id
            );
        }
    }

    #[test]
    fn test_environment_validation_integration() {
        let mut provider = MockEnvironmentProvider::new();

        // Test that validation works end-to-end with multiple invalid values
        provider
            .set("CNI_COMMAND", "ADD")
            .set("CNI_NETNS", "/invalid/path") // Invalid netns
            .set("CNI_IFNAME", "invalid interface name"); // Invalid ifname (space)

        let cni_env = CniEnvironment::load(&provider);
        let result = cni_env.validate_for_command();
        assert!(result.is_err());

        // Fix netns, should still fail on ifname
        provider.set("CNI_NETNS", "/proc/123/ns/net");
        let cni_env = CniEnvironment::load(&provider);
        let result = cni_env.validate_for_command();
        assert!(result.is_err());

        // Fix ifname, should now pass
        provider.set("CNI_IFNAME", "eth0");
        let cni_env = CniEnvironment::load(&provider);
        let result = cni_env.validate_for_command();
        assert!(result.is_ok());

        // Test DEL command with all requirements
        provider
            .set("CNI_COMMAND", "DEL")
            .set("CNI_CONTAINERID", "valid-container-123");
        let cni_env = CniEnvironment::load(&provider);
        let result = cni_env.validate_for_command();
        assert!(result.is_ok());
    }
}
