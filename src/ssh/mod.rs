/// Módulo SSH para gerenciamento de conexões e execução de comandos Docker remotos

pub mod config;
pub mod connection;
pub mod client;

// Re-exports para facilitar o uso
pub use config::{SshConfig, SshServerConfig};
pub use connection::SshConnection;
pub use client::SshDockerClient;

/// Placeholder para funcionalidades SSH
/// Este módulo será expandido conforme necessário
pub struct SshManager {
    config: SshConfig,
}

impl SshManager {
    pub fn new() -> Self {
        Self {
            config: SshConfig::default(),
        }
    }

    pub fn get_config(&self) -> &SshConfig {
        &self.config
    }
}

impl Default for SshManager {
    fn default() -> Self {
        Self::new()
    }
}