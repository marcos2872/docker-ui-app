use super::config::SshServerConfig;
use anyhow::Result;
use std::collections::HashMap;

/// Cliente SSH para execução de comandos Docker remotos
#[derive(Debug)]
pub struct SshDockerClient {
    server_name: String,
}

/// Informações do servidor Docker remoto
#[derive(Debug, Clone)]
pub struct RemoteDockerInfo {
    pub server_name: String,
    pub docker_version: String,
    pub api_version: String,
    pub os: String,
    pub architecture: String,
    pub containers_running: usize,
    pub containers_total: usize,
    pub images_count: usize,
}

impl SshDockerClient {
    /// Cria novo cliente SSH Docker
    pub fn new(config: SshServerConfig) -> Self {
        let server_name = config.name.clone();
        
        Self {
            server_name,
        }
    }

    /// Obtém nome do servidor
    pub fn get_server_name(&self) -> &str {
        &self.server_name
    }
}

/// Gerenciador de múltiplos clientes SSH
pub struct SshClientManager {
    clients: HashMap<String, SshDockerClient>,
}

impl SshClientManager {
    /// Cria novo gerenciador de clientes SSH
    pub fn new() -> Self {
        Self {
            clients: HashMap::new(),
        }
    }

    /// Adiciona cliente SSH
    pub fn add_client(&mut self, config: SshServerConfig) -> Result<()> {
        let client = SshDockerClient::new(config.clone());
        self.clients.insert(config.name.clone(), client);
        Ok(())
    }

    /// Remove cliente SSH
    pub fn remove_client(&mut self, server_name: &str) -> bool {
        self.clients.remove(server_name).is_some()
    }

    /// Lista todos os clientes
    pub fn list_clients(&self) -> Vec<String> {
        self.clients.keys().cloned().collect()
    }
}

impl Default for SshClientManager {
    fn default() -> Self {
        Self::new()
    }
}