use std::error::Error;
use std::fmt;

/// Root error type for all CNI-gesprek operations
#[derive(Debug)]
pub enum CniError {
    /// Configuration-related errors
    Config(ConfigError),
    /// Environment variable errors
    Environment(EnvironmentError),
    /// Command execution errors
    Command(CommandError),
}

impl fmt::Display for CniError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CniError::Config(e) => write!(f, "Configuration error: {}", e),
            CniError::Environment(e) => write!(f, "Environment error: {}", e),
            CniError::Command(e) => write!(f, "Command error: {}", e),
        }
    }
}

impl Error for CniError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            CniError::Config(e) => Some(e),
            CniError::Environment(e) => Some(e),
            CniError::Command(e) => Some(e),
        }
    }
}

/// Configuration-related errors
#[derive(Debug)]
pub enum ConfigError {
    /// Missing required CNI configuration
    MissingConfig { command: String, reason: String },
    /// JSON parsing errors
    ParseError { message: String },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::MissingConfig { command, reason } => {
                write!(
                    f,
                    "Missing configuration for command '{}': {}",
                    command, reason
                )
            }
            ConfigError::ParseError { message } => {
                write!(f, "Failed to parse CNI configuration: {}", message)
            }
        }
    }
}

impl Error for ConfigError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}

/// Environment variable errors
#[derive(Debug)]
pub enum EnvironmentError {
    /// Missing required environment variable
    MissingVariable { variable: String, command: String },
    /// Invalid environment variable value (simplified version)
    InvalidValue {
        name: String,
        value: String,
        expected: String,
    },
}

impl fmt::Display for EnvironmentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EnvironmentError::MissingVariable { variable, command } => {
                write!(
                    f,
                    "Environment variable '{}' required for command '{}' is not set",
                    variable, command
                )
            }
            EnvironmentError::InvalidValue {
                name,
                value,
                expected,
            } => {
                write!(
                    f,
                    "Environment variable '{}' has invalid value '{}', expected: {}",
                    name, value, expected
                )
            }
        }
    }
}

impl Error for EnvironmentError {}

/// Command execution errors
#[derive(Debug)]
pub enum CommandError {
    /// Unknown CNI command
    UnknownCommand { command: String },
    /// Network namespace not found
    NetnsNotFound { netns: String },
    /// Interface not found in network namespace
    InterfaceNotFound { interface: String, netns: String },
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommandError::UnknownCommand { command } => {
                write!(f, "Unknown CNI command: {}", command)
            }
            CommandError::NetnsNotFound { netns } => {
                write!(f, "Network namespace does not exist: {}", netns)
            }
            CommandError::InterfaceNotFound { interface, netns } => {
                write!(
                    f,
                    "Interface '{}' not found in network namespace '{}'",
                    interface, netns
                )
            }
        }
    }
}

impl Error for CommandError {}

/// Convenience type alias for CNI results
pub type CniResult<T> = Result<T, CniError>;

// Essential From implementations for error conversion
impl From<serde_json::Error> for CniError {
    fn from(e: serde_json::Error) -> Self {
        CniError::Config(ConfigError::ParseError {
            message: format!("JSON error: {}", e),
        })
    }
}

impl From<ConfigError> for CniError {
    fn from(e: ConfigError) -> Self {
        CniError::Config(e)
    }
}

impl From<EnvironmentError> for CniError {
    fn from(e: EnvironmentError) -> Self {
        CniError::Environment(e)
    }
}

impl From<CommandError> for CniError {
    fn from(e: CommandError) -> Self {
        CniError::Command(e)
    }
}
    (missing $command:expr, $reason:expr) => {
        CniError::Config(ConfigError::MissingConfig {
            command: $command.to_string(),
            reason: $reason.to_string(),
        })
    };
    (invalid $field:expr, $value:expr, $reason:expr) => {
        CniError::Config(ConfigError::InvalidConfig {
            field: $field.to_string(),
            value: $value.to_string(),
            reason: $reason.to_string(),
        })
    };
    (json $source:expr, $context:expr) => {
        CniError::Config(ConfigError::JsonParse {
            source: $source,
            context: $context.to_string(),
        })
    };
}

#[macro_export]
macro_rules! network_error {
    (interface $interface:expr, $operation:expr, $reason:expr) => {
        CniError::Network(NetworkError::InterfaceError {
            interface: $interface.to_string(),
            operation: $operation.to_string(),
            reason: $reason.to_string(),
        })
    };
    (ip $cidr:expr, $operation:expr, $reason:expr) => {
        CniError::Network(NetworkError::IpAddressError {
            cidr: $cidr.to_string(),
            operation: $operation.to_string(),
            reason: $reason.to_string(),
        })
    };
    (namespace $namespace:expr, $operation:expr, $reason:expr) => {
        CniError::Network(NetworkError::NamespaceError {
            namespace: $namespace.to_string(),
            operation: $operation.to_string(),
            reason: $reason.to_string(),
        })
    };
}

#[macro_export]
macro_rules! env_error {
    (missing $variable:expr, $command:expr) => {
        CniError::Environment(EnvironmentError::MissingVariable {
            variable: $variable.to_string(),
            command: $command.to_string(),
        })
    };
    (unknown_command $command:expr, $supported:expr) => {
        CniError::Environment(EnvironmentError::UnknownCommand {
            command: $command.to_string(),
            supported_commands: $supported,
        })
    };
}

// Conversion from legacy string errors for backward compatibility
impl From<&str> for CniError {
    fn from(s: &str) -> Self {
        CniError::System(SystemError::NotFound {
            resource: s.to_string(),
            resource_type: "unknown".to_string(),
        })
    }
}

impl From<String> for CniError {
    fn from(s: String) -> Self {
        CniError::System(SystemError::NotFound {
            resource: s,
            resource_type: "unknown".to_string(),
        })
    }
}

impl From<std::io::Error> for CniError {
    fn from(e: std::io::Error) -> Self {
        CniError::System(SystemError::Io {
            source: e,
            context: "unknown operation".to_string(),
        })
    }
}

impl From<serde_json::Error> for CniError {
    fn from(e: serde_json::Error) -> Self {
        CniError::Config(ConfigError::ParseError {
            message: format!("JSON error: {}", e),
        })
    }
}

impl From<ConfigError> for CniError {
    fn from(e: ConfigError) -> Self {
        CniError::Config(e)
    }
}

impl From<EnvironmentError> for CniError {
    fn from(e: EnvironmentError) -> Self {
        CniError::Environment(e)
    }
}

impl From<CommandError> for CniError {
    fn from(e: CommandError) -> Self {
        CniError::Command(e)
    }
}

impl From<NetworkError> for CniError {
    fn from(e: NetworkError) -> Self {
        CniError::Network(e)
    }
}

impl From<ValidationError> for CniError {
    fn from(e: ValidationError) -> Self {
        CniError::Validation(e)
    }
}

impl From<SystemError> for CniError {
    fn from(e: SystemError) -> Self {
        CniError::System(e)
    }
}

impl From<Box<dyn std::error::Error>> for CniError {
    fn from(e: Box<dyn std::error::Error>) -> Self {
        CniError::System(SystemError::Generic {
            message: format!("{}", e),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_structured_error_display() {
        let config_err = CniError::Config(ConfigError::MissingConfig {
            command: "ADD".to_string(),
            reason: "Required for network setup".to_string(),
        });

        let display = format!("{}", config_err);
        assert!(display.contains("Configuration error"));
        assert!(display.contains("Missing configuration for command 'ADD'"));
    }

    #[test]
    fn test_error_macros() {
        let err = config_error!(missing "ADD", "Test reason");
        match err {
            CniError::Config(ConfigError::MissingConfig { command, reason }) => {
                assert_eq!(command, "ADD");
                assert_eq!(reason, "Test reason");
            }
            _ => panic!("Wrong error type"),
        }
    }

    #[test]
    fn test_network_error_context() {
        let err = network_error!(interface "eth0", "create", "Permission denied");
        let display = format!("{}", err);
        assert!(display.contains("Interface 'eth0' operation 'create' failed: Permission denied"));
    }

    #[test]
    fn test_error_chaining() {
        let json_err = serde_json::from_str::<serde_json::Value>("invalid").unwrap_err();
        let cni_err = CniError::from(json_err);

        assert!(cni_err.source().is_some());
        match cni_err {
            CniError::Config(ConfigError::ParseError { .. }) => {}
            _ => panic!("Wrong error type"),
        }
    }
}
