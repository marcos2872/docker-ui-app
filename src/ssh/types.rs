use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshConnection {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub private_key: Option<String>,
    pub passphrase: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResult {
    pub command: String,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub success: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshServerInfo {
    pub hostname: String,
    pub uptime: String,
    pub memory_usage: String,
    pub cpu_usage: String,
    pub disk_usage: String,
}

impl Default for SshConnection {
    fn default() -> Self {
        Self {
            host: String::new(),
            port: 22,
            username: String::new(),
            password: String::new(),
            private_key: None,
            passphrase: None,
        }
    }
}
