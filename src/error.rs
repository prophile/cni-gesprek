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

impl From<std::io::Error> for CniError {
    fn from(e: std::io::Error) -> Self {
        CniError::Config(ConfigError::ParseError {
            message: format!("IO error: {}", e),
        })
    }
}

impl From<Box<dyn std::error::Error>> for CniError {
    fn from(e: Box<dyn std::error::Error>) -> Self {
        CniError::Config(ConfigError::ParseError {
            message: format!("General error: {}", e),
        })
    }
}
