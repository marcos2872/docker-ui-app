use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use crate::remote::{ServerInfo, RemoteServerManager};
use crate::ssh::SshConfig;

/// Configuração geral da aplicação
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Configuração SSH
    pub ssh_config: SshConfig,
    /// Servidores configurados
    pub servers: Vec<ServerInfo>,
    /// ID do servidor ativo
    pub active_server_id: Option<String>,
    /// Configurações da interface
    pub ui_config: UiConfig,
    /// Versão da configuração para compatibilidade
    pub config_version: String,
}

/// Configurações da interface do usuário
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    /// Tema (dark/light)
    pub theme: String,
    /// Idioma
    pub language: String,
    /// Refresh automático (em segundos)
    pub auto_refresh_interval: u64,
    /// Mostrar containers do sistema
    pub show_system_containers: bool,
    /// Logs por padrão
    pub default_log_lines: usize,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            ssh_config: SshConfig::default(),
            servers: Vec::new(),
            active_server_id: None,
            ui_config: UiConfig::default(),
            config_version: "1.0".to_string(),
        }
    }
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            theme: "dark".to_string(),
            language: "pt_BR".to_string(),
            auto_refresh_interval: 5,
            show_system_containers: false,
            default_log_lines: 50,
        }
    }
}

/// Gerenciador de configuração da aplicação
pub struct ConfigManager {
    config_path: PathBuf,
    config: AppConfig,
}

impl ConfigManager {
    /// Cria novo gerenciador de configuração
    pub fn new() -> Result<Self> {
        let config_path = Self::get_config_path()?;
        let config = Self::load_config(&config_path).unwrap_or_default();

        Ok(Self {
            config_path,
            config,
        })
    }

    /// Obtém o caminho do arquivo de configuração
    fn get_config_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .context("Não foi possível encontrar diretório de configuração")?
            .join("docker-ui-app");

        // Cria o diretório se não existir
        fs::create_dir_all(&config_dir)
            .context("Falha ao criar diretório de configuração")?;

        Ok(config_dir.join("config.json"))
    }

    /// Carrega configuração do arquivo
    fn load_config(path: &PathBuf) -> Result<AppConfig> {
        if !path.exists() {
            return Ok(AppConfig::default());
        }

        let content = fs::read_to_string(path)
            .context("Falha ao ler arquivo de configuração")?;

        let config: AppConfig = serde_json::from_str(&content)
            .context("Falha ao deserializar configuração")?;

        Ok(config)
    }

    /// Salva configuração no arquivo
    pub fn save_config(&self) -> Result<()> {
        let content = serde_json::to_string_pretty(&self.config)
            .context("Falha ao serializar configuração")?;

        fs::write(&self.config_path, content)
            .context("Falha ao salvar arquivo de configuração")?;

        Ok(())
    }

    /// Obtém referência para a configuração
    pub fn get_config(&self) -> &AppConfig {
        &self.config
    }

    /// Obtém referência mutável para a configuração
    pub fn get_config_mut(&mut self) -> &mut AppConfig {
        &mut self.config
    }

    /// Define servidor ativo
    pub fn set_active_server(&mut self, server_id: Option<String>) -> Result<()> {
        self.config.active_server_id = server_id;
        self.save_config()
    }

    /// Obtém servidor ativo
    pub fn get_active_server(&self) -> Option<&String> {
        self.config.active_server_id.as_ref()
    }

    /// Adiciona servidor à configuração
    pub fn add_server(&mut self, server: ServerInfo) -> Result<()> {
        // Remove servidor existente com mesmo ID
        self.config.servers.retain(|s| s.id != server.id);
        
        // Adiciona novo servidor
        self.config.servers.push(server);
        
        self.save_config()
    }

    /// Remove servidor da configuração
    pub fn remove_server(&mut self, server_id: &str) -> Result<bool> {
        let initial_len = self.config.servers.len();
        self.config.servers.retain(|s| s.id != server_id);
        
        // Se era o servidor ativo, remove referência
        if let Some(ref active_id) = self.config.active_server_id {
            if active_id == server_id {
                self.config.active_server_id = None;
            }
        }
        
        let removed = self.config.servers.len() != initial_len;
        if removed {
            self.save_config()?;
        }
        
        Ok(removed)
    }

    /// Lista todos os servidores
    pub fn list_servers(&self) -> &Vec<ServerInfo> {
        &self.config.servers
    }

    /// Obtém servidor por ID
    pub fn get_server(&self, server_id: &str) -> Option<&ServerInfo> {
        self.config.servers.iter().find(|s| s.id == server_id)
    }

    /// Atualiza informações de um servidor
    pub fn update_server(&mut self, server: ServerInfo) -> Result<()> {
        if let Some(existing) = self.config.servers.iter_mut().find(|s| s.id == server.id) {
            *existing = server;
            self.save_config()?;
        }
        Ok(())
    }

    /// Exporta configuração de servidores para JSON
    pub fn export_servers(&self) -> Result<String> {
        serde_json::to_string_pretty(&self.config.servers)
            .context("Falha ao exportar configuração de servidores")
    }

    /// Importa configuração de servidores do JSON
    pub fn import_servers(&mut self, json_data: &str) -> Result<usize> {
        let imported_servers: Vec<ServerInfo> = serde_json::from_str(json_data)
            .context("Falha ao importar configuração de servidores")?;

        let mut added_count = 0;
        for server in imported_servers {
            // Verifica se servidor já existe
            if !self.config.servers.iter().any(|s| s.id == server.id) {
                self.config.servers.push(server);
                added_count += 1;
            }
        }

        if added_count > 0 {
            self.save_config()?;
        }

        Ok(added_count)
    }

    /// Limpa todos os servidores
    pub fn clear_servers(&mut self) -> Result<usize> {
        let count = self.config.servers.len();
        self.config.servers.clear();
        self.config.active_server_id = None;
        self.save_config()?;
        Ok(count)
    }

    /// Atualiza configuração da UI
    pub fn update_ui_config(&mut self, ui_config: UiConfig) -> Result<()> {
        self.config.ui_config = ui_config;
        self.save_config()
    }

    /// Inicializa RemoteServerManager com configuração salva
    pub async fn init_server_manager(&self) -> Result<RemoteServerManager> {
        let manager = RemoteServerManager::new();

        // Carrega servidores salvos
        for server in &self.config.servers {
            // Para servidores remotos, adiciona usando SSH config
            if let crate::remote::ServerType::Remote(ssh_config) = &server.server_type {
                manager.add_remote_server(
                    server.name.clone(),
                    ssh_config.clone(),
                    server.description.clone(),
                ).await?;
            } else {
                // Para servidores locais
                manager.add_local_server(
                    server.name.clone(),
                    server.description.clone(),
                ).await?;
            }
        }

        // Define servidor ativo se configurado
        if let Some(active_id) = &self.config.active_server_id {
            if let Err(e) = manager.set_active_server(active_id).await {
                eprintln!("Aviso: Não foi possível definir servidor ativo '{}': {}", active_id, e);
            }
        }

        Ok(manager)
    }

    /// Sincroniza estado do RemoteServerManager com a configuração
    pub async fn sync_with_server_manager(&mut self, manager: &RemoteServerManager) -> Result<()> {
        // Obtém lista atualizada de servidores
        let servers = manager.list_servers().await;
        self.config.servers = servers;

        // Obtém servidor ativo
        self.config.active_server_id = manager.get_active_server().await;

        self.save_config()
    }

    /// Obtém estatísticas da configuração
    pub fn get_stats(&self) -> ConfigStats {
        ConfigStats {
            total_servers: self.config.servers.len(),
            remote_servers: self.config.servers.iter()
                .filter(|s| matches!(s.server_type, crate::remote::ServerType::Remote(_)))
                .count(),
            local_servers: self.config.servers.iter()
                .filter(|s| matches!(s.server_type, crate::remote::ServerType::Local))
                .count(),
            has_active_server: self.config.active_server_id.is_some(),
            config_version: self.config.config_version.clone(),
        }
    }

    /// Salva automaticamente as configurações
    pub fn auto_save(&self) -> Result<()> {
        if let Err(e) = self.save_config() {
            eprintln!("Erro ao salvar configurações automaticamente: {}", e);
            return Err(e);
        }
        println!("Configurações salvas automaticamente");
        Ok(())
    }

    /// Obtém configuração completa para exportação
    pub fn export_full_config(&self) -> Result<String> {
        serde_json::to_string_pretty(&self.config)
            .context("Falha ao serializar configuração para exportação")
    }

    /// Importa configuração completa
    pub fn import_full_config(&mut self, json_data: &str) -> Result<()> {
        let imported_config: AppConfig = serde_json::from_str(json_data)
            .context("Falha ao deserializar configuração importada")?;
        
        self.config = imported_config;
        self.save_config()?;
        println!("Configuração importada com sucesso");
        Ok(())
    }

    /// Obtém último servidor usado
    pub fn get_last_active_server(&self) -> Option<&String> {
        self.config.active_server_id.as_ref()
    }

    /// Define servidor ativo e salva
    pub fn set_and_save_active_server(&mut self, server_id: Option<String>) -> Result<()> {
        self.config.active_server_id = server_id;
        self.auto_save()
    }
}

/// Estatísticas da configuração
#[derive(Debug, Clone)]
pub struct ConfigStats {
    pub total_servers: usize,
    pub remote_servers: usize,
    pub local_servers: usize,
    pub has_active_server: bool,
    pub config_version: String,
}

impl Default for ConfigManager {
    fn default() -> Self {
        Self::new().expect("Falha ao criar ConfigManager")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use crate::ssh::{SshServerConfig, AuthMethod};
    use std::path::PathBuf;

    fn create_test_config_manager() -> (ConfigManager, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.json");
        
        let manager = ConfigManager {
            config_path,
            config: AppConfig::default(),
        };
        
        (manager, temp_dir)
    }

    #[test]
    fn test_default_config() {
        let config = AppConfig::default();
        assert_eq!(config.config_version, "1.0");
        assert!(config.servers.is_empty());
        assert!(config.active_server_id.is_none());
    }

    #[test]
    fn test_save_and_load_config() {
        let (mut manager, _temp_dir) = create_test_config_manager();
        
        // Modifica configuração
        manager.config.ui_config.theme = "light".to_string();
        
        // Salva
        manager.save_config().unwrap();
        
        // Carrega novamente
        let loaded_config = ConfigManager::load_config(&manager.config_path).unwrap();
        assert_eq!(loaded_config.ui_config.theme, "light");
    }

    #[test]
    fn test_server_management() {
        let (mut manager, _temp_dir) = create_test_config_manager();
        
        let ssh_config = SshServerConfig::new_with_password(
            "test_server".to_string(),
            "localhost".to_string(),
            22,
            "user".to_string(),
            "password".to_string(),
        );

        let server = ServerInfo {
            id: "test-1".to_string(),
            name: "Test Server".to_string(),
            server_type: crate::remote::ServerType::Remote(ssh_config),
            description: Some("Test".to_string()),
            is_active: false,
            is_connected: false,
            last_connected: None,
            docker_version: None,
            containers_count: 0,
            images_count: 0,
            networks_count: 0,
            volumes_count: 0,
        };

        // Adiciona servidor
        manager.add_server(server.clone()).unwrap();
        assert_eq!(manager.list_servers().len(), 1);

        // Remove servidor
        assert!(manager.remove_server(&server.id).unwrap());
        assert_eq!(manager.list_servers().len(), 0);
    }

    #[test]
    fn test_export_import_servers() {
        let (mut manager, _temp_dir) = create_test_config_manager();
        
        let ssh_config = SshServerConfig::new_with_password(
            "test_server".to_string(),
            "localhost".to_string(),
            22,
            "user".to_string(),
            "password".to_string(),
        );

        let server = ServerInfo {
            id: "test-1".to_string(),
            name: "Test Server".to_string(),
            server_type: crate::remote::ServerType::Remote(ssh_config),
            description: Some("Test".to_string()),
            is_active: false,
            is_connected: false,
            last_connected: None,
            docker_version: None,
            containers_count: 0,
            images_count: 0,
            networks_count: 0,
            volumes_count: 0,
        };

        manager.add_server(server).unwrap();

        // Exporta
        let exported = manager.export_servers().unwrap();
        assert!(!exported.is_empty());

        // Limpa e importa
        manager.clear_servers().unwrap();
        assert_eq!(manager.list_servers().len(), 0);

        let imported_count = manager.import_servers(&exported).unwrap();
        assert_eq!(imported_count, 1);
        assert_eq!(manager.list_servers().len(), 1);
    }
}