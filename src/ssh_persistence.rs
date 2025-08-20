use crate::ssh::SshConnection;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedSshServer {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    #[serde(skip_serializing_if = "String::is_empty", default)]
    pub password: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub private_key_path: Option<String>,
    pub last_connected: Option<chrono::DateTime<chrono::Utc>>,
    pub is_favorite: bool,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshServerConfig {
    pub servers: Vec<SavedSshServer>,
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

impl Default for SshServerConfig {
    fn default() -> Self {
        Self {
            servers: Vec::new(),
            last_updated: chrono::Utc::now(),
        }
    }
}

impl SavedSshServer {
    pub fn new(name: String, host: String, username: String, port: Option<u16>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            host,
            port: port.unwrap_or(22),
            username,
            password: String::new(),
            private_key_path: None,
            last_connected: None,
            is_favorite: false,
            description: None,
        }
    }

    pub fn to_ssh_connection(&self) -> SshConnection {
        SshConnection {
            host: self.host.clone(),
            port: self.port,
            username: self.username.clone(),
            password: self.password.clone(),
            private_key: self.private_key_path.clone(),
            passphrase: None,
        }
    }

    pub fn update_last_connected(&mut self) {
        self.last_connected = Some(chrono::Utc::now());
    }
}

pub struct SshPersistence {
    config_path: PathBuf,
}

impl SshPersistence {
    pub fn new() -> Result<Self> {
        let config_dir = dirs::config_dir()
            .or_else(|| dirs::home_dir().map(|h| h.join(".config")))
            .context("Cannot determine config directory")?
            .join("docker-ui");

        if !config_dir.exists() {
            fs::create_dir_all(&config_dir).context("Failed to create config directory")?;
        }

        Ok(Self {
            config_path: config_dir.join("ssh_servers.json"),
        })
    }

    pub fn load_config(&self) -> Result<SshServerConfig> {
        if !self.config_path.exists() {
            return Ok(SshServerConfig::default());
        }

        let content =
            fs::read_to_string(&self.config_path).context("Failed to read SSH config file")?;

        let config: SshServerConfig =
            serde_json::from_str(&content).context("Failed to parse SSH config file")?;

        Ok(config)
    }

    pub fn save_config(&self, config: &SshServerConfig) -> Result<()> {
        let mut config = config.clone();
        config.last_updated = chrono::Utc::now();

        let content =
            serde_json::to_string_pretty(&config).context("Failed to serialize SSH config")?;

        fs::write(&self.config_path, content).context("Failed to write SSH config file")?;

        Ok(())
    }

    pub fn add_server(&self, server: SavedSshServer) -> Result<()> {
        let mut config = self.load_config()?;

        // Remove se já existe um servidor com o mesmo ID
        config.servers.retain(|s| s.id != server.id);

        config.servers.push(server);
        self.save_config(&config)
    }

    pub fn update_server(&self, server: SavedSshServer) -> Result<()> {
        let mut config = self.load_config()?;

        if let Some(existing) = config.servers.iter_mut().find(|s| s.id == server.id) {
            *existing = server;
            self.save_config(&config)?;
        } else {
            return Err(anyhow::anyhow!("Server not found"));
        }

        Ok(())
    }

    pub fn remove_server(&self, server_id: &str) -> Result<()> {
        let mut config = self.load_config()?;

        let initial_count = config.servers.len();
        config.servers.retain(|s| s.id != server_id);

        if config.servers.len() == initial_count {
            return Err(anyhow::anyhow!("Server not found"));
        }

        self.save_config(&config)
    }

    pub fn get_server(&self, server_id: &str) -> Result<Option<SavedSshServer>> {
        let config = self.load_config()?;
        Ok(config.servers.into_iter().find(|s| s.id == server_id))
    }

    pub fn list_servers(&self) -> Result<Vec<SavedSshServer>> {
        let config = self.load_config()?;
        Ok(config.servers)
    }

    pub fn get_favorites(&self) -> Result<Vec<SavedSshServer>> {
        let config = self.load_config()?;
        Ok(config
            .servers
            .into_iter()
            .filter(|s| s.is_favorite)
            .collect())
    }

    pub fn mark_as_connected(&self, server_id: &str) -> Result<()> {
        let mut config = self.load_config()?;

        if let Some(server) = config.servers.iter_mut().find(|s| s.id == server_id) {
            server.update_last_connected();
            self.save_config(&config)?;
        }

        Ok(())
    }

    pub fn toggle_favorite(&self, server_id: &str) -> Result<bool> {
        let mut config = self.load_config()?;

        if let Some(server) = config.servers.iter_mut().find(|s| s.id == server_id) {
            server.is_favorite = !server.is_favorite;
            let is_favorite = server.is_favorite;
            self.save_config(&config)?;
            Ok(is_favorite)
        } else {
            Err(anyhow::anyhow!("Server not found"))
        }
    }

    pub fn search_servers(&self, query: &str) -> Result<Vec<SavedSshServer>> {
        let config = self.load_config()?;
        let query = query.to_lowercase();

        Ok(config
            .servers
            .into_iter()
            .filter(|s| {
                s.name.to_lowercase().contains(&query)
                    || s.host.to_lowercase().contains(&query)
                    || s.username.to_lowercase().contains(&query)
                    || s.description
                        .as_ref()
                        .map_or(false, |d| d.to_lowercase().contains(&query))
            })
            .collect())
    }

    pub fn get_recent_servers(&self, limit: usize) -> Result<Vec<SavedSshServer>> {
        let config = self.load_config()?;
        let mut servers = config.servers;

        // Ordena por última conexão, mais recente primeiro
        servers.sort_by(|a, b| match (a.last_connected, b.last_connected) {
            (Some(a_time), Some(b_time)) => b_time.cmp(&a_time),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a.name.cmp(&b.name),
        });

        servers.truncate(limit);
        Ok(servers)
    }

    pub fn export_config(&self, export_path: &Path) -> Result<()> {
        let config = self.load_config()?;
        let content = serde_json::to_string_pretty(&config)?;
        fs::write(export_path, content)?;
        Ok(())
    }

    pub fn import_config(&self, import_path: &Path, merge: bool) -> Result<usize> {
        let content = fs::read_to_string(import_path)?;
        let imported_config: SshServerConfig = serde_json::from_str(&content)?;

        let mut config = if merge {
            self.load_config()?
        } else {
            SshServerConfig::default()
        };

        let mut added_count = 0;

        for server in imported_config.servers {
            if !config
                .servers
                .iter()
                .any(|s| s.host == server.host && s.username == server.username)
            {
                config.servers.push(server);
                added_count += 1;
            }
        }

        self.save_config(&config)?;
        Ok(added_count)
    }
}
