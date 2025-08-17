use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Configuração de servidor SSH para conexão remota
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SshServerConfig {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_method: AuthMethod,
    pub docker_socket: Option<String>, // Caminho customizado do socket Docker
}

/// Método de autenticação SSH (apenas senha)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AuthMethod {
    /// Senha para autenticação SSH
    pub password: String,
}

/// Configuração global SSH
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SshConfig {
    pub servers: Vec<SshServerConfig>,
    pub default_server: Option<String>,
    pub connection_timeout: u64, // em segundos
    pub command_timeout: u64,    // em segundos
}

impl Default for SshConfig {
    fn default() -> Self {
        Self {
            servers: Vec::new(),
            default_server: None,
            connection_timeout: 30,
            command_timeout: 60,
        }
    }
}

impl SshServerConfig {
    /// Cria uma nova configuração de servidor com autenticação por senha
    pub fn new(
        name: String,
        host: String,
        port: u16,
        username: String,
        password: String,
    ) -> Self {
        Self {
            name,
            host,
            port,
            username,
            auth_method: AuthMethod { password },
            docker_socket: None,
        }
    }

    /// Define um caminho customizado para o socket Docker
    pub fn with_docker_socket(mut self, socket_path: String) -> Self {
        self.docker_socket = Some(socket_path);
        self
    }

    /// Retorna o caminho do socket Docker (padrão ou customizado)
    pub fn get_docker_socket(&self) -> String {
        self.docker_socket
            .clone()
            .unwrap_or_else(|| "unix:///var/run/docker.sock".to_string())
    }

    /// Valida se a configuração está válida
    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("Nome do servidor não pode estar vazio".to_string());
        }

        if self.host.trim().is_empty() {
            return Err("Host não pode estar vazio".to_string());
        }

        if self.port == 0 {
            return Err("Porta deve ser maior que 0".to_string());
        }

        if self.username.trim().is_empty() {
            return Err("Username não pode estar vazio".to_string());
        }

        // Valida senha
        if self.auth_method.password.trim().is_empty() {
            return Err("Senha não pode estar vazia".to_string());
        }

        Ok(())
    }
}

impl SshConfig {
    /// Carrega configuração de um arquivo JSON
    pub fn load_from_file(path: &PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let config: SshConfig = serde_json::from_str(&content)?;
        Ok(config)
    }

    /// Salva configuração em um arquivo JSON
    pub fn save_to_file(&self, path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
        let content = serde_json::to_string_pretty(self)?;
        
        // Cria diretório pai se não existir
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Adiciona um novo servidor à configuração
    pub fn add_server(&mut self, server: SshServerConfig) -> Result<(), String> {
        server.validate()?;

        // Verifica se já existe servidor com mesmo nome
        if self.servers.iter().any(|s| s.name == server.name) {
            return Err(format!("Servidor com nome '{}' já existe", server.name));
        }

        self.servers.push(server);
        Ok(())
    }

    /// Remove um servidor da configuração
    pub fn remove_server(&mut self, name: &str) -> bool {
        let initial_len = self.servers.len();
        self.servers.retain(|s| s.name != name);
        
        // Se era o servidor padrão, remove a referência
        if let Some(ref default) = self.default_server {
            if default == name {
                self.default_server = None;
            }
        }
        
        self.servers.len() != initial_len
    }

    /// Busca um servidor por nome
    pub fn get_server(&self, name: &str) -> Option<&SshServerConfig> {
        self.servers.iter().find(|s| s.name == name)
    }

    /// Busca um servidor por nome (mutável)
    pub fn get_server_mut(&mut self, name: &str) -> Option<&mut SshServerConfig> {
        self.servers.iter_mut().find(|s| s.name == name)
    }

    /// Retorna o servidor padrão
    pub fn get_default_server(&self) -> Option<&SshServerConfig> {
        if let Some(ref name) = self.default_server {
            self.get_server(name)
        } else {
            self.servers.first()
        }
    }

    /// Define o servidor padrão
    pub fn set_default_server(&mut self, name: &str) -> Result<(), String> {
        if self.servers.iter().any(|s| s.name == name) {
            self.default_server = Some(name.to_string());
            Ok(())
        } else {
            Err(format!("Servidor '{}' não encontrado", name))
        }
    }

    /// Retorna o caminho padrão do arquivo de configuração
    pub fn default_config_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
        let mut path = dirs::config_dir()
            .ok_or("Não foi possível determinar diretório de configuração")?;
        path.push("docker-ui");
        path.push("ssh-config.json");
        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_ssh_server_config_validation() {
        let config = SshServerConfig::new_with_password(
            "test".to_string(),
            "localhost".to_string(),
            22,
            "user".to_string(),
            "password".to_string(),
        );
        
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_ssh_server_config_empty_name() {
        let config = SshServerConfig::new_with_password(
            "".to_string(),
            "localhost".to_string(),
            22,
            "user".to_string(),
            "password".to_string(),
        );
        
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_ssh_config_add_server() {
        let mut config = SshConfig::default();
        let server = SshServerConfig::new_with_password(
            "test".to_string(),
            "localhost".to_string(),
            22,
            "user".to_string(),
            "password".to_string(),
        );
        
        assert!(config.add_server(server).is_ok());
        assert_eq!(config.servers.len(), 1);
    }

    #[test]
    fn test_ssh_config_duplicate_server() {
        let mut config = SshConfig::default();
        let server1 = SshServerConfig::new_with_password(
            "test".to_string(),
            "localhost".to_string(),
            22,
            "user".to_string(),
            "password".to_string(),
        );
        let server2 = SshServerConfig::new_with_password(
            "test".to_string(),
            "localhost".to_string(),
            22,
            "user".to_string(),
            "password".to_string(),
        );
        
        assert!(config.add_server(server1).is_ok());
        assert!(config.add_server(server2).is_err());
    }
}