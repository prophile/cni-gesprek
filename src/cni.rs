use serde::{Deserialize, Serialize};

use crate::environment::{CniEnvironment, EnvironmentProvider};
use crate::error::{CniResult, ConfigError};

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
    pub fn load<E: EnvironmentProvider>(env_provider: &E, args_dry_run: bool) -> CniResult<Self> {
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
    fn parse_stdin_config(command: &str) -> CniResult<Option<CniConfig>> {
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
                    let config: CniConfig =
                        serde_json::from_str(&buffer).map_err(|e| ConfigError::ParseError {
                            message: format!("Failed to parse CNI configuration: {}", e),
                        })?;
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
    pub fn require_config(&self) -> CniResult<&CniConfig> {
        self.config.as_ref().ok_or_else(|| {
            ConfigError::MissingConfig {
                command: self.environment.command.clone(),
                reason: "Missing CNI configuration on stdin".to_string(),
            }
            .into()
        })
    }
}

impl TryFrom<&CniEnvironment> for CniContext {
    type Error = crate::error::CniError;

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

    #[test]
    fn test_cni_context_creation() {
        let context = CniContext {
            environment: CniEnvironment {
                command: "VERSION".to_string(),
                container_id: Some("test-container".to_string()),
                netns: Some("/var/run/netns/test".to_string()),
                ifname: Some("eth0".to_string()),
            },
            config: None,
            is_dry_run: false,
        };

        assert_eq!(context.environment.command, "VERSION");
        assert!(!context.is_dry_run);
        assert!(context.config.is_none());
    }

    #[test]
    fn test_cni_context_dry_run() {
        let context = CniContext {
            environment: CniEnvironment {
                command: "ADD".to_string(),
                container_id: Some("test-container".to_string()),
                netns: Some("/var/run/netns/test".to_string()),
                ifname: Some("eth0".to_string()),
            },
            config: None,
            is_dry_run: true,
        };

        assert!(context.is_dry_run);
    }

    #[test]
    fn test_cni_context_commands() {
        // Test ADD command context
        let add_context = CniContext {
            environment: CniEnvironment {
                command: "ADD".to_string(),
                container_id: Some("test".to_string()),
                netns: Some("/test".to_string()),
                ifname: Some("eth0".to_string()),
            },
            config: None,
            is_dry_run: false,
        };
        assert_eq!(add_context.environment.command, "ADD");

        // Test VERSION command context
        let version_context = CniContext {
            environment: CniEnvironment {
                command: "VERSION".to_string(),
                container_id: Some("test".to_string()),
                netns: Some("/test".to_string()),
                ifname: Some("eth0".to_string()),
            },
            config: None,
            is_dry_run: false,
        };
        assert_eq!(version_context.environment.command, "VERSION");
    }
}
