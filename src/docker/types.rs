use serde::{Deserialize, Serialize};
use std::fmt;

// Informações básicas de um container
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ContainerInfo {
    pub id: String,
    pub name: String,
    pub image: String,
    pub state: String,
    pub status: String,
    pub ports: String,  // Mudando para String para incluir formato completo como "8080:80/tcp"
    pub ports_list: Vec<i32>,  // Mantendo lista numérica para compatibilidade
    pub created: i64,
    pub running_for: String,  // Novo campo para tempo de execução
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ImageInfo {
    pub id: String,
    pub tags: Vec<String>,
    pub created: i64,
    pub size: i64,
    pub in_use: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NetworkInfo {
    pub id: String,
    pub name: String,
    pub driver: String,
    pub scope: String,
    pub created: String,
    pub containers_count: i32,
    pub is_system: bool,
    pub in_use: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VolumeInfo {
    pub name: String,
    pub driver: String,
    pub mountpoint: String,
    pub created: String,
    pub containers_count: i32,
    pub in_use: bool,
}

// Status possíveis do Docker
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum DockerStatus {
    Running,
    NotRunning,
    NotInstalled,
    PermissionDenied,
}

impl fmt::Display for DockerStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DockerStatus::Running => write!(f, "Rodando"),
            DockerStatus::NotRunning => write!(f, "Não está rodando"),
            DockerStatus::NotInstalled => write!(f, "Não instalado"),
            DockerStatus::PermissionDenied => write!(f, "Permissão negada"),
        }
    }
}

// Informações gerais do sistema Docker
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DockerInfo {
    pub version: String,
    pub containers: i64,
    pub containers_paused: i64,
    pub containers_running: i64,
    pub containers_stopped: i64,
    pub images: i64,
    pub architecture: String,
}

// Uso total do sistema Docker
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DockerSystemUsage {
    pub cpu_online: u64,
    pub cpu_usage: f64,
    pub memory_usage: u64,
    pub memory_limit: u64,
    pub memory_percentage: f64,
    pub network_rx_bytes: u64,
    pub network_tx_bytes: u64,
    pub block_read_bytes: u64,
    pub block_write_bytes: u64,
    pub containers_stats: Vec<ContainerStats>,
}

// Estatísticas detalhadas de um container
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ContainerStats {
    pub id: String,
    pub name: String,
    pub cpu_percentage: f64,
    pub memory_usage: u64,
    pub memory_limit: u64,
    pub memory_percentage: f64,
    pub network_rx: u64,
    pub network_tx: u64,
    pub block_read: u64,
    pub block_write: u64,
}

// Estrutura para criar um novo container
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CreateContainerRequest {
    pub name: String,
    pub image: String,
    pub ports: Vec<PortMapping>,
    pub volumes: Vec<VolumeMapping>,
    pub environment: Vec<EnvVar>,
    pub command: Option<String>,
    pub restart_policy: String,
}

// Mapeamento de portas
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PortMapping {
    pub host_port: u16,
    pub container_port: u16,
    pub protocol: String, // tcp ou udp
}

// Mapeamento de volumes
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VolumeMapping {
    pub host_path: String,
    pub container_path: String,
    pub read_only: bool,
}

// Variável de ambiente
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EnvVar {
    pub key: String,
    pub value: String,
}
