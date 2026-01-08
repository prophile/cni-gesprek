use crate::error::{CniResult, EnvironmentError};
use regex::Regex;
use std::env;

/// Trait for abstracting environment variable access
pub trait EnvironmentProvider {
    /// Get an environment variable value
    fn get(&self, key: &str) -> Option<String>;

    /// Check if an environment variable exists (regardless of value)
    fn exists(&self, key: &str) -> bool {
        self.get(key).is_some()
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

/// CNI-specific environment variables container
#[derive(Debug, Clone)]
pub struct CniEnvironment {
    pub command: String,
    pub netns: Option<String>,
    pub ifname: Option<String>,
    pub container_id: Option<String>,
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
        let provider = SystemEnvironmentProvider;

        // Test getting an environment variable that should exist
        let path = provider.get("PATH");
        assert!(path.is_some());

        // Test getting a non-existent variable
        let nonexistent = provider.get("NONEXISTENT_VAR_12345");
        assert!(nonexistent.is_none());
    }

    #[test]
    fn test_cni_environment_creation() {
        // Test basic CniEnvironment structure
        let cni_env = CniEnvironment {
            command: "ADD".to_string(),
            netns: Some("/proc/123/ns/net".to_string()),
            ifname: Some("eth0".to_string()),
            container_id: Some("container123".to_string()),
        };

        assert_eq!(cni_env.command, "ADD");
        assert_eq!(cni_env.netns, Some("/proc/123/ns/net".to_string()));
        assert_eq!(cni_env.ifname, Some("eth0".to_string()));
        assert_eq!(cni_env.container_id, Some("container123".to_string()));
    }

    #[test]
    fn test_cni_environment_getters() {
        let cni_env = CniEnvironment {
            command: "ADD".to_string(),
            netns: Some("/proc/123/ns/net".to_string()),
            ifname: Some("eth0".to_string()),
            container_id: Some("container123".to_string()),
        };

        assert_eq!(cni_env.get_netns().unwrap(), "/proc/123/ns/net");
        assert_eq!(cni_env.get_ifname().unwrap(), "eth0");
        assert_eq!(cni_env.get_container_id().unwrap(), "container123");

        // Test error cases with None values
        let empty_env = CniEnvironment {
            command: "VERSION".to_string(),
            netns: None,
            ifname: None,
            container_id: None,
        };

        assert!(empty_env.get_netns().is_err());
        assert!(empty_env.get_ifname().is_err());
        assert!(empty_env.get_container_id().is_err());
    }
}
