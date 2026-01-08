use std::error::Error;

use serde::{Deserialize, Serialize};

use crate::environment::{CniEnvironment, EnvironmentProvider};

/// CNI configuration structure from JSON input
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CniConfig {
    #[serde(rename = "cniVersion")]
    pub cni_version: String,
    pub name: String,
    #[serde(rename = "type")]
    pub plugin_type: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub master: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "podCIDR")]
    pub pod_cidr: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub dns: Option<CniDns>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CniDns {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nameservers: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<Vec<String>>,
}

/// Comprehensive CNI context that consolidates all CNI-specific data
/// This replaces the scattered parsing in main() with a structured approach
#[derive(Debug, Clone)]
pub struct CniContext {
    pub environment: CniEnvironment,
    pub config: Option<CniConfig>,
    pub is_dry_run: bool,
}

impl CniContext {
    /// Create a new CniContext by loading environment and parsing configuration
    pub fn load<E: EnvironmentProvider>(
        env_provider: &E,
        args_dry_run: bool,
    ) -> Result<Self, Box<dyn Error>> {
        // Load CNI environment variables
        let environment = CniEnvironment::load(env_provider);

        // Validate environment variables for the command early
        environment.validate_for_command()?;

        // Check for DRY_RUN environment variable or command line flag
        let is_dry_run = args_dry_run || env_provider.exists("DRY_RUN");

        // Parse JSON configuration from stdin if needed for this command
        let config = Self::parse_stdin_config(&environment.command)?;

        Ok(Self {
            environment,
            config,
            is_dry_run,
        })
    }

    /// Parse JSON configuration from stdin for commands that require it
    fn parse_stdin_config(command: &str) -> Result<Option<CniConfig>, Box<dyn Error>> {
        if ["ADD", "DEL", "CHECK", "STATUS"].contains(&command) {
            // In tests, we don't want to actually read from stdin
            #[cfg(test)]
            {
                return Ok(None);
            }

            #[cfg(not(test))]
            {
                use std::io::{self, Read};

                let mut buffer = String::new();
                let bytes_read = io::stdin().read_to_string(&mut buffer)?;

                if bytes_read > 0 && !buffer.trim().is_empty() {
                    let config: CniConfig = serde_json::from_str(&buffer)
                        .map_err(|e| format!("Failed to parse CNI configuration: {}", e))?;
                    Ok(Some(config))
                } else {
                    Ok(None)
                }
            }
        } else {
            Ok(None)
        }
    }

    /// Get the configuration, returning an error if it's required but missing
    pub fn require_config(&self) -> Result<&CniConfig, Box<dyn Error>> {
        self.config
            .as_ref()
            .ok_or("Missing CNI configuration on stdin".into())
    }

    /// Check if this is a command that requires configuration
    pub fn needs_config(&self) -> bool {
        ["ADD", "DEL", "CHECK", "STATUS"].contains(&self.environment.command.as_str())
    }

    /// Validate that the configuration is available if needed
    pub fn validate_config(&self) -> Result<(), Box<dyn Error>> {
        if self.needs_config() && self.config.is_none() {
            return Err("Configuration required for this CNI command but none provided".into());
        }
        Ok(())
    }
}

impl TryFrom<&CniEnvironment> for CniContext {
    type Error = Box<dyn Error>;

    fn try_from(environment: &CniEnvironment) -> Result<Self, Self::Error> {
        // Parse configuration if needed
        let config = Self::parse_stdin_config(&environment.command)?;

        Ok(Self {
            environment: environment.clone(),
            config,
            is_dry_run: false, // Default, can be overridden
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::MockEnvironmentProvider;

    #[test]
    fn test_cni_context_creation() {
        let mut env_provider = MockEnvironmentProvider::new();
        env_provider
            .set("CNI_COMMAND", "VERSION")
            .set("CNI_CONTAINERID", "test-container")
            .set("CNI_NETNS", "/var/run/netns/test")
            .set("CNI_IFNAME", "eth0");

        let context = CniContext::load(&env_provider, false).unwrap();

        assert_eq!(context.environment.command, "VERSION");
        assert!(!context.is_dry_run);
        assert!(context.config.is_none()); // VERSION command doesn't need config
    }

    #[test]
    fn test_cni_context_dry_run() {
        let mut env_provider = MockEnvironmentProvider::new();
        env_provider
            .set("CNI_COMMAND", "ADD")
            .set("CNI_CONTAINERID", "test-container")
            .set("CNI_NETNS", "/var/run/netns/test")
            .set("CNI_IFNAME", "eth0")
            .set("DRY_RUN", "true");

        let context = CniContext::load(&env_provider, false).unwrap();

        assert!(context.is_dry_run);
    }

    #[test]
    fn test_config_validation() {
        let mut env_provider = MockEnvironmentProvider::new();
        env_provider
            .set("CNI_COMMAND", "VERSION")
            .set("CNI_CONTAINERID", "test-container")
            .set("CNI_NETNS", "/var/run/netns/test")
            .set("CNI_IFNAME", "eth0");

        let context = CniContext::load(&env_provider, false).unwrap();

        // VERSION command doesn't need config, so validation should pass
        assert!(context.validate_config().is_ok());
        assert!(!context.needs_config());
    }

    #[test]
    fn test_needs_config_detection() {
        assert!(CniContext {
            environment: CniEnvironment {
                command: "ADD".to_string(),
                container_id: Some("test".to_string()),
                netns: Some("/test".to_string()),
                ifname: Some("eth0".to_string()),
                path: None,
            },
            config: None,
            is_dry_run: false
        }
        .needs_config());

        assert!(!CniContext {
            environment: CniEnvironment {
                command: "VERSION".to_string(),
                container_id: Some("test".to_string()),
                netns: Some("/test".to_string()),
                ifname: Some("eth0".to_string()),
                path: None,
            },
            config: None,
            is_dry_run: false
        }
        .needs_config());
    }
}
