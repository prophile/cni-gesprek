use std::collections::HashMap;
use std::env;
use std::error::Error;

/// Trait for abstracting environment variable access
pub trait EnvironmentProvider {
    /// Get an environment variable value
    fn get(&self, key: &str) -> Option<String>;

    /// Get an environment variable, returning an error if not set
    fn get_required(&self, key: &str) -> Result<String, Box<dyn Error>> {
        self.get(key)
            .ok_or_else(|| format!("{} not set", key).into())
    }

    /// Check if an environment variable exists (regardless of value)
    fn exists(&self, key: &str) -> bool {
        self.get(key).is_some()
    }

    /// Get multiple required environment variables at once
    fn get_required_multiple(
        &self,
        keys: &[&str],
    ) -> Result<HashMap<String, String>, Box<dyn Error>> {
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

    /// Validate that all required environment variables are present for a given command
    pub fn validate_for_command(&self) -> Result<(), Box<dyn Error>> {
        match self.command.as_str() {
            "ADD" | "DEL" | "CHECK" => {
                if self.netns.is_none() {
                    return Err("CNI_NETNS not set".into());
                }
                if self.ifname.is_none() {
                    return Err("CNI_IFNAME not set".into());
                }
                if self.container_id.is_none() && (self.command == "DEL" || self.command == "CHECK")
                {
                    return Err("CNI_CONTAINERID not set".into());
                }
            }
            "VERSION" | "STATUS" | "GC" => {
                // These commands don't require additional environment variables
            }
            _ => {
                return Err(format!("Unknown CNI command: {}", self.command).into());
            }
        }
        Ok(())
    }

    /// Get the network namespace path, returning an error if not set
    pub fn get_netns(&self) -> Result<&str, Box<dyn Error>> {
        self.netns
            .as_ref()
            .map(|s| s.as_str())
            .ok_or("CNI_NETNS not set".into())
    }

    /// Get the interface name, returning an error if not set
    pub fn get_ifname(&self) -> Result<&str, Box<dyn Error>> {
        self.ifname
            .as_ref()
            .map(|s| s.as_str())
            .ok_or("CNI_IFNAME not set".into())
    }

    /// Get the container ID, returning an error if not set
    pub fn get_container_id(&self) -> Result<&str, Box<dyn Error>> {
        self.container_id
            .as_ref()
            .map(|s| s.as_str())
            .ok_or("CNI_CONTAINERID not set".into())
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
}
