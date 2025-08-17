use crate::ssh::{SshConfig, SshServerConfig, SshDockerClient};
// Docker types imported but not used yet - will be needed for full implementation
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Tipo de servidor (local ou remoto)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ServerType {
    Local,
    Remote(SshServerConfig),
}

/// Informações de um servidor gerenciado
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    pub id: String,
    pub name: String,
    pub server_type: ServerType,
    pub description: Option<String>,
    pub is_active: bool,
    pub last_connected: Option<String>,
    pub docker_version: Option<String>,
    pub containers_count: usize,
    pub images_count: usize,
    pub networks_count: usize,
    pub volumes_count: usize,
}

/// Status de conectividade de um servidor
#[derive(Debug, Clone)]
pub enum ServerStatus {
    Connected,
    Disconnected,
    Connecting,
    Error(String),
}

/// Resultado de operação em servidor
#[derive(Debug, Clone)]
pub struct ServerOperationResult {
    pub server_id: String,
    pub success: bool,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

/// Gerenciador de múltiplos servidores Docker
pub struct RemoteServerManager {
    servers: Arc<RwLock<HashMap<String, ServerInfo>>>,
    ssh_clients: Arc<RwLock<HashMap<String, Arc<SshDockerClient>>>>,
    active_server: Arc<RwLock<Option<String>>>,
    ssh_config: SshConfig,
}

impl RemoteServerManager {
    /// Cria novo gerenciador de servidores
    pub fn new() -> Self {
        Self {
            servers: Arc::new(RwLock::new(HashMap::new())),
            ssh_clients: Arc::new(RwLock::new(HashMap::new())),
            active_server: Arc::new(RwLock::new(None)),
            ssh_config: SshConfig::default(),
        }
    }

    /// Cria gerenciador com configuração SSH existente
    pub fn with_ssh_config(ssh_config: SshConfig) -> Self {
        Self {
            servers: Arc::new(RwLock::new(HashMap::new())),
            ssh_clients: Arc::new(RwLock::new(HashMap::new())),
            active_server: Arc::new(RwLock::new(None)),
            ssh_config,
        }
    }

    /// Adiciona servidor local
    pub async fn add_local_server(&self, name: String, description: Option<String>) -> Result<String> {
        let server_id = format!("local_{}", uuid::Uuid::new_v4().to_string()[..8].to_string());
        
        let server_info = ServerInfo {
            id: server_id.clone(),
            name,
            server_type: ServerType::Local,
            description,
            is_active: true,
            last_connected: Some(chrono::Utc::now().to_rfc3339()),
            docker_version: None, // Será preenchido ao conectar
            containers_count: 0,
            images_count: 0,
            networks_count: 0,
            volumes_count: 0,
        };

        let mut servers = self.servers.write().await;
        servers.insert(server_id.clone(), server_info);

        // Se for o primeiro servidor, torna-o ativo
        let mut active = self.active_server.write().await;
        if active.is_none() {
            *active = Some(server_id.clone());
        }

        Ok(server_id)
    }

    /// Adiciona servidor remoto via SSH
    pub async fn add_remote_server(
        &self,
        name: String,
        ssh_config: SshServerConfig,
        description: Option<String>,
    ) -> Result<String> {
        // Valida configuração SSH
        ssh_config.validate()
            .map_err(|e| anyhow::anyhow!("Configuração SSH inválida: {}", e))?;

        let server_id = format!("remote_{}", uuid::Uuid::new_v4().to_string()[..8].to_string());
        
        let server_info = ServerInfo {
            id: server_id.clone(),
            name,
            server_type: ServerType::Remote(ssh_config.clone()),
            description,
            is_active: false, // Começa inativo até conectar
            last_connected: None,
            docker_version: None,
            containers_count: 0,
            images_count: 0,
            networks_count: 0,
            volumes_count: 0,
        };

        // Cria cliente SSH
        let ssh_client = Arc::new(SshDockerClient::new(ssh_config));

        let mut servers = self.servers.write().await;
        let mut clients = self.ssh_clients.write().await;
        
        servers.insert(server_id.clone(), server_info);
        clients.insert(server_id.clone(), ssh_client);

        Ok(server_id)
    }

    /// Remove servidor
    pub async fn remove_server(&self, server_id: &str) -> Result<bool> {
        let mut servers = self.servers.write().await;
        let mut clients = self.ssh_clients.write().await;
        let mut active = self.active_server.write().await;

        // Remove das estruturas
        let removed = servers.remove(server_id).is_some();
        clients.remove(server_id);

        // Se era o servidor ativo, remove referência
        if let Some(ref active_id) = *active {
            if active_id == server_id {
                *active = None;
            }
        }

        Ok(removed)
    }

    /// Lista todos os servidores
    pub async fn list_servers(&self) -> Vec<ServerInfo> {
        let servers = self.servers.read().await;
        servers.values().cloned().collect()
    }

    /// Obtém informações de um servidor específico
    pub async fn get_server(&self, server_id: &str) -> Option<ServerInfo> {
        let servers = self.servers.read().await;
        servers.get(server_id).cloned()
    }

    /// Obtém servidor ativo
    pub async fn get_active_server(&self) -> Option<String> {
        let active = self.active_server.read().await;
        active.clone()
    }

    /// Define servidor ativo
    pub async fn set_active_server(&self, server_id: &str) -> Result<()> {
        let servers = self.servers.read().await;
        
        if !servers.contains_key(server_id) {
            return Err(anyhow::anyhow!("Servidor '{}' não encontrado", server_id));
        }

        let mut active = self.active_server.write().await;
        *active = Some(server_id.to_string());

        Ok(())
    }

    /// Conecta a um servidor específico
    pub async fn connect_to_server(&self, server_id: &str) -> Result<()> {
        let server_info = {
            let servers = self.servers.read().await;
            servers.get(server_id).cloned()
                .ok_or_else(|| anyhow::anyhow!("Servidor '{}' não encontrado", server_id))?
        };

        match server_info.server_type {
            ServerType::Local => {
                // Para servidor local, apenas marca como ativo
                let mut servers = self.servers.write().await;
                if let Some(server) = servers.get_mut(server_id) {
                    server.is_active = true;
                    server.last_connected = Some(chrono::Utc::now().to_rfc3339());
                }
                Ok(())
            }
            ServerType::Remote(_) => {
                // Para servidor remoto, tenta conectar via SSH
                let clients = self.ssh_clients.read().await;
                if let Some(_client) = clients.get(server_id) {
                    // Placeholder - implementação SSH real viria aqui
                    // client.connect().await?;
                    
                    let mut servers = self.servers.write().await;
                    if let Some(server) = servers.get_mut(server_id) {
                        server.is_active = true;
                        server.last_connected = Some(chrono::Utc::now().to_rfc3339());
                    }
                    Ok(())
                } else {
                    Err(anyhow::anyhow!("Cliente SSH não encontrado para servidor '{}'", server_id))
                }
            }
        }
    }

    /// Desconecta de um servidor específico
    pub async fn disconnect_from_server(&self, server_id: &str) -> Result<()> {
        let server_info = {
            let servers = self.servers.read().await;
            servers.get(server_id).cloned()
                .ok_or_else(|| anyhow::anyhow!("Servidor '{}' não encontrado", server_id))?
        };

        match server_info.server_type {
            ServerType::Local => {
                // Para servidor local, apenas marca como inativo
                let mut servers = self.servers.write().await;
                if let Some(server) = servers.get_mut(server_id) {
                    server.is_active = false;
                }
                Ok(())
            }
            ServerType::Remote(_) => {
                // Para servidor remoto, desconecta SSH
                let clients = self.ssh_clients.read().await;
                if let Some(_client) = clients.get(server_id) {
                    // Placeholder - implementação SSH real viria aqui
                    // client.disconnect().await?;
                    
                    let mut servers = self.servers.write().await;
                    if let Some(server) = servers.get_mut(server_id) {
                        server.is_active = false;
                    }
                    Ok(())
                } else {
                    Err(anyhow::anyhow!("Cliente SSH não encontrado para servidor '{}'", server_id))
                }
            }
        }
    }

    /// Conecta a todos os servidores
    pub async fn connect_all(&self) -> Vec<ServerOperationResult> {
        let server_ids: Vec<String> = {
            let servers = self.servers.read().await;
            servers.keys().cloned().collect()
        };

        let mut results = Vec::new();
        for server_id in server_ids {
            let result = match self.connect_to_server(&server_id).await {
                Ok(_) => ServerOperationResult {
                    server_id: server_id.clone(),
                    success: true,
                    message: "Conectado com sucesso".to_string(),
                    data: None,
                },
                Err(e) => ServerOperationResult {
                    server_id: server_id.clone(),
                    success: false,
                    message: e.to_string(),
                    data: None,
                },
            };
            results.push(result);
        }

        results
    }

    /// Desconecta de todos os servidores
    pub async fn disconnect_all(&self) -> Vec<ServerOperationResult> {
        let server_ids: Vec<String> = {
            let servers = self.servers.read().await;
            servers.keys().cloned().collect()
        };

        let mut results = Vec::new();
        for server_id in server_ids {
            let result = match self.disconnect_from_server(&server_id).await {
                Ok(_) => ServerOperationResult {
                    server_id: server_id.clone(),
                    success: true,
                    message: "Desconectado com sucesso".to_string(),
                    data: None,
                },
                Err(e) => ServerOperationResult {
                    server_id: server_id.clone(),
                    success: false,
                    message: e.to_string(),
                    data: None,
                },
            };
            results.push(result);
        }

        results
    }

    /// Testa conectividade de um servidor
    pub async fn test_server_connectivity(&self, server_id: &str) -> Result<bool> {
        let server_info = {
            let servers = self.servers.read().await;
            servers.get(server_id).cloned()
                .ok_or_else(|| anyhow::anyhow!("Servidor '{}' não encontrado", server_id))?
        };

        match server_info.server_type {
            ServerType::Local => {
                // Para servidor local, sempre retorna true se Docker estiver disponível
                Ok(true) // Placeholder - verificaria Docker local
            }
            ServerType::Remote(_) => {
                // Para servidor remoto, testa conectividade SSH
                let clients = self.ssh_clients.read().await;
                if let Some(_client) = clients.get(server_id) {
                    // Placeholder - implementação SSH real viria aqui
                    // client.test_connection().await
                    Ok(false) // Por ora retorna false
                } else {
                    Ok(false)
                }
            }
        }
    }

    /// Obtém status de todos os servidores
    pub async fn get_servers_status(&self) -> HashMap<String, ServerStatus> {
        let server_ids: Vec<String> = {
            let servers = self.servers.read().await;
            servers.keys().cloned().collect()
        };

        let mut status_map = HashMap::new();
        for server_id in server_ids {
            let status = match self.test_server_connectivity(&server_id).await {
                Ok(true) => ServerStatus::Connected,
                Ok(false) => ServerStatus::Disconnected,
                Err(e) => ServerStatus::Error(e.to_string()),
            };
            status_map.insert(server_id, status);
        }

        status_map
    }

    /// Atualiza informações de um servidor
    pub async fn update_server_info(&self, server_id: &str, docker_version: Option<String>, 
                                   containers_count: usize, images_count: usize,
                                   networks_count: usize, volumes_count: usize) -> Result<()> {
        let mut servers = self.servers.write().await;
        
        if let Some(server) = servers.get_mut(server_id) {
            server.docker_version = docker_version;
            server.containers_count = containers_count;
            server.images_count = images_count;
            server.networks_count = networks_count;
            server.volumes_count = volumes_count;
            server.last_connected = Some(chrono::Utc::now().to_rfc3339());
            Ok(())
        } else {
            Err(anyhow::anyhow!("Servidor '{}' não encontrado", server_id))
        }
    }

    /// Exporta configuração de servidores
    pub async fn export_servers_config(&self) -> Result<String> {
        let servers = self.servers.read().await;
        let servers_vec: Vec<&ServerInfo> = servers.values().collect();
        
        serde_json::to_string_pretty(&servers_vec)
            .context("Falha ao serializar configuração de servidores")
    }

    /// Importa configuração de servidores
    pub async fn import_servers_config(&self, json_data: &str) -> Result<usize> {
        let imported_servers: Vec<ServerInfo> = serde_json::from_str(json_data)
            .context("Falha ao deserializar configuração de servidores")?;

        let mut added_count = 0;
        let mut servers = self.servers.write().await;
        let mut clients = self.ssh_clients.write().await;

        for server_info in imported_servers {
            // Verifica se servidor já existe
            if servers.contains_key(&server_info.id) {
                continue; // Pula se já existe
            }

            // Adiciona servidor
            servers.insert(server_info.id.clone(), server_info.clone());

            // Se for servidor remoto, cria cliente SSH
            if let ServerType::Remote(ssh_config) = &server_info.server_type {
                let ssh_client = Arc::new(SshDockerClient::new(ssh_config.clone()));
                clients.insert(server_info.id.clone(), ssh_client);
            }

            added_count += 1;
        }

        Ok(added_count)
    }

    /// Limpa todos os servidores
    pub async fn clear_all_servers(&self) -> Result<usize> {
        let mut servers = self.servers.write().await;
        let mut clients = self.ssh_clients.write().await;
        let mut active = self.active_server.write().await;

        let count = servers.len();
        servers.clear();
        clients.clear();
        *active = None;

        Ok(count)
    }

    /// Obtém cliente SSH para servidor remoto
    pub async fn get_ssh_client(&self, server_id: &str) -> Option<Arc<SshDockerClient>> {
        let clients = self.ssh_clients.read().await;
        clients.get(server_id).cloned()
    }

    /// Lista apenas servidores ativos
    pub async fn list_active_servers(&self) -> Vec<ServerInfo> {
        let servers = self.servers.read().await;
        servers.values()
            .filter(|server| server.is_active)
            .cloned()
            .collect()
    }

    /// Lista apenas servidores remotos
    pub async fn list_remote_servers(&self) -> Vec<ServerInfo> {
        let servers = self.servers.read().await;
        servers.values()
            .filter(|server| matches!(server.server_type, ServerType::Remote(_)))
            .cloned()
            .collect()
    }

    /// Lista apenas servidores locais
    pub async fn list_local_servers(&self) -> Vec<ServerInfo> {
        let servers = self.servers.read().await;
        servers.values()
            .filter(|server| matches!(server.server_type, ServerType::Local))
            .cloned()
            .collect()
    }

    /// Obtém estatísticas gerais
    pub async fn get_statistics(&self) -> HashMap<String, usize> {
        let servers = self.servers.read().await;
        
        let total_servers = servers.len();
        let active_servers = servers.values().filter(|s| s.is_active).count();
        let remote_servers = servers.values().filter(|s| matches!(s.server_type, ServerType::Remote(_))).count();
        let local_servers = servers.values().filter(|s| matches!(s.server_type, ServerType::Local)).count();
        
        let total_containers: usize = servers.values().map(|s| s.containers_count).sum();
        let total_images: usize = servers.values().map(|s| s.images_count).sum();
        let total_networks: usize = servers.values().map(|s| s.networks_count).sum();
        let total_volumes: usize = servers.values().map(|s| s.volumes_count).sum();

        let mut stats = HashMap::new();
        stats.insert("total_servers".to_string(), total_servers);
        stats.insert("active_servers".to_string(), active_servers);
        stats.insert("remote_servers".to_string(), remote_servers);
        stats.insert("local_servers".to_string(), local_servers);
        stats.insert("total_containers".to_string(), total_containers);
        stats.insert("total_images".to_string(), total_images);
        stats.insert("total_networks".to_string(), total_networks);
        stats.insert("total_volumes".to_string(), total_volumes);

        stats
    }
}

impl Default for RemoteServerManager {
    fn default() -> Self {
        Self::new()
    }
}

// Adiciona dependência uuid ao Cargo.toml se necessário
// Essa implementação usar uuid para gerar IDs únicos dos servidores

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ssh::AuthMethod;
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_create_manager() {
        let manager = RemoteServerManager::new();
        let servers = manager.list_servers().await;
        assert_eq!(servers.len(), 0);
    }

    #[tokio::test]
    async fn test_add_local_server() {
        let manager = RemoteServerManager::new();
        
        let server_id = manager.add_local_server(
            "Local Docker".to_string(),
            Some("Servidor Docker local".to_string())
        ).await.unwrap();
        
        let servers = manager.list_servers().await;
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].name, "Local Docker");
        
        // Verifica se foi definido como ativo
        let active = manager.get_active_server().await;
        assert_eq!(active, Some(server_id));
    }

    #[tokio::test]
    async fn test_add_remote_server() {
        let manager = RemoteServerManager::new();
        
        let ssh_config = SshServerConfig::new_with_password(
            "test_server".to_string(),
            "localhost".to_string(),
            22,
            "user".to_string(),
            "password".to_string(),
        );
        
        let server_id = manager.add_remote_server(
            "Remote Docker".to_string(),
            ssh_config,
            Some("Servidor Docker remoto".to_string())
        ).await.unwrap();
        
        let servers = manager.list_servers().await;
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].name, "Remote Docker");
        
        // Verifica se é do tipo remoto
        match &servers[0].server_type {
            ServerType::Remote(_) => assert!(true),
            _ => assert!(false, "Deveria ser servidor remoto"),
        }
    }

    #[tokio::test]
    async fn test_remove_server() {
        let manager = RemoteServerManager::new();
        
        let server_id = manager.add_local_server(
            "Test Server".to_string(),
            None
        ).await.unwrap();
        
        assert_eq!(manager.list_servers().await.len(), 1);
        
        let removed = manager.remove_server(&server_id).await.unwrap();
        assert!(removed);
        assert_eq!(manager.list_servers().await.len(), 0);
    }
}