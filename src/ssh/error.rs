use std::fmt;

#[derive(Debug)]
#[allow(dead_code)]
pub enum SshError {
    ConnectionFailed(String),
    AuthenticationFailed(String),
    CommandExecutionFailed(String),
    FileTransferFailed(String),
    NetworkError(String),
    InvalidCredentials(String),
    Timeout(String),
}

impl fmt::Display for SshError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SshError::ConnectionFailed(msg) => write!(f, "Connection failed: {}", msg),
            SshError::AuthenticationFailed(msg) => write!(f, "Authentication failed: {}", msg),
            SshError::CommandExecutionFailed(msg) => write!(f, "Command execution failed: {}", msg),
            SshError::FileTransferFailed(msg) => write!(f, "File transfer failed: {}", msg),
            SshError::NetworkError(msg) => write!(f, "Network error: {}", msg),
            SshError::InvalidCredentials(msg) => write!(f, "Invalid credentials: {}", msg),
            SshError::Timeout(msg) => write!(f, "Timeout: {}", msg),
        }
    }
}

impl std::error::Error for SshError {}

impl From<ssh2::Error> for SshError {
    fn from(error: ssh2::Error) -> Self {
        match error.code() {
            ssh2::ErrorCode::Session(-18) => SshError::AuthenticationFailed(error.message().to_string()),
            ssh2::ErrorCode::Session(-2) => SshError::ConnectionFailed(error.message().to_string()),
            _ => SshError::NetworkError(error.message().to_string()),
        }
    }
}

impl From<std::io::Error> for SshError {
    fn from(error: std::io::Error) -> Self {
        SshError::NetworkError(error.to_string())
    }
}