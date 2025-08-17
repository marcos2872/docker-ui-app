// Imports para gerenciamento do Docker
use anyhow::{Context, Result};
use bollard::{
    Docker,
    models::ContainerCreateBody,
    models::{ContainerStatsResponse, ImageSummary},
    query_parameters::CreateContainerOptions,
    query_parameters::{
        ListContainersOptions, ListImagesOptions, ListNetworksOptions, ListVolumesOptions,
        RestartContainerOptions, StatsOptions,
    },
};
use chrono;
use futures_util::TryStreamExt;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fmt,
    process::Command,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

// Imports dos módulos remotos
use crate::remote::{DockerRemoteAdapter, ServerInfo, ServerType, RemoteServerManager};
use crate::ssh::SshServerConfig;

// Informações básicas de um container
#[derive(Debug, Serialize, Deserialize)]
pub struct ContainerInfo {
    pub id: String,
    pub name: String,
    pub image: String,
    pub state: String,
    pub status: String,
    pub ports: Vec<i32>,
    pub created: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ImageInfo {
    pub id: String,
    pub tags: Vec<String>,
    pub created: i64,
    pub size: i64,
    pub in_use: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NetworkInfo {
    pub id: String,
    pub name: String,
    pub driver: String,
    pub scope: String,
    pub created: String,
    pub containers_count: i32,
    pub is_system: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VolumeInfo {
    pub name: String,
    pub driver: String,
    pub mountpoint: String,
    pub created: String,
    pub containers_count: i32,
}

// Status possíveis do Docker
#[derive(Debug, Serialize, Deserialize)]
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

// Cache para estatísticas anteriores (necessário para cálculo de delta)
#[derive(Debug, Clone)]
#[allow(dead_code)] // Alguns campos podem ser usados no futuro
struct PreviousStats {
    timestamp: u64,
    cpu_total: u64,
    system_total: u64,
    network_rx: u64,
    network_tx: u64,
    block_read: u64,
    block_write: u64,
}

/// Contexto de execução do Docker (local ou remoto)
#[derive(Debug, Clone)]
pub enum DockerExecutionContext {
    Local(Docker),
    Remote {
        adapter: Arc<DockerRemoteAdapter>,
        server_info: ServerInfo,
    },
}

/// Configuração do contexto Docker
#[derive(Debug, Clone)]
pub struct DockerContextConfig {
    pub server_id: String,
    pub server_name: String,
    pub is_remote: bool,
    pub ssh_config: Option<SshServerConfig>,
}

// Gerenciador principal do Docker com suporte a contextos local e remoto
pub struct DockerManager {
    context: DockerExecutionContext,
    context_config: DockerContextConfig,
    previous_stats: HashMap<String, PreviousStats>,
    server_manager: Arc<RemoteServerManager>,
}

// Informações gerais do sistema Docker
#[derive(Debug, Serialize, Deserialize)]
pub struct DockerInfo {
    pub version: String,
    pub containers: i64,
    pub containers_paused: i64,
    pub containers_running: i64,
    pub containers_stopped: i64,
    pub images: i64,
    pub architecture: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CpuCalculate {
    pub usage_cpu: f64,
    pub online_cpus: u64,
}

// Uso total do sistema Docker
#[derive(Debug, Serialize, Deserialize)]
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
#[derive(Debug, Serialize, Deserialize)]
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

impl DockerManager {
    /// Cria nova instância conectando ao Docker daemon local
    pub async fn new() -> Result<Self> {
        let docker = Docker::connect_with_socket_defaults()
            .context("Falha ao conectar com Docker daemon")?;

        let context_config = DockerContextConfig {
            server_id: "local".to_string(),
            server_name: "Local".to_string(),
            is_remote: false,
            ssh_config: None,
        };

        let server_manager = Arc::new(RemoteServerManager::new());

        Ok(DockerManager {
            context: DockerExecutionContext::Local(docker),
            context_config,
            previous_stats: HashMap::new(),
            server_manager,
        })
    }

    /// Cria nova instância com gerenciador de servidores existente
    pub async fn new_with_server_manager(server_manager: Arc<RemoteServerManager>) -> Result<Self> {
        let docker = Docker::connect_with_socket_defaults()
            .context("Falha ao conectar com Docker daemon")?;

        let context_config = DockerContextConfig {
            server_id: "local".to_string(),
            server_name: "Local".to_string(),
            is_remote: false,
            ssh_config: None,
        };

        Ok(DockerManager {
            context: DockerExecutionContext::Local(docker),
            context_config,
            previous_stats: HashMap::new(),
            server_manager,
        })
    }

    /// Cria instância para servidor remoto via SSH
    pub async fn new_remote(
        server_info: ServerInfo,
        server_manager: Arc<RemoteServerManager>,
    ) -> Result<Self> {
        let ssh_config = match &server_info.server_type {
            ServerType::Remote(config) => config.clone(),
            ServerType::Local => {
                return Err(anyhow::anyhow!("Servidor não é do tipo remoto"));
            }
        };

        let adapter = Arc::new(DockerRemoteAdapter::new(ssh_config.clone()));
        
        // Tenta conectar ao servidor remoto
        adapter.connect().await
            .context("Falha ao conectar ao servidor Docker remoto")?;

        let context_config = DockerContextConfig {
            server_id: server_info.id.clone(),
            server_name: server_info.name.clone(),
            is_remote: true,
            ssh_config: Some(ssh_config),
        };

        Ok(DockerManager {
            context: DockerExecutionContext::Remote {
                adapter,
                server_info,
            },
            context_config,
            previous_stats: HashMap::new(),
            server_manager,
        })
    }

    /// Troca o contexto para um servidor específico
    pub async fn switch_to_server(&mut self, server_id: &str) -> Result<()> {
        if server_id == "local" {
            // Troca para contexto local
            let docker = Docker::connect_with_socket_defaults()
                .context("Falha ao conectar com Docker daemon local")?;

            self.context = DockerExecutionContext::Local(docker);
            self.context_config = DockerContextConfig {
                server_id: "local".to_string(),
                server_name: "Local".to_string(),
                is_remote: false,
                ssh_config: None,
            };
        } else {
            // Troca para contexto remoto
            let server_info = self.server_manager.get_server(server_id).await
                .ok_or_else(|| anyhow::anyhow!("Servidor '{}' não encontrado", server_id))?;

            let ssh_config = match &server_info.server_type {
                ServerType::Remote(config) => config.clone(),
                ServerType::Local => {
                    return Err(anyhow::anyhow!("Servidor não é do tipo remoto"));
                }
            };

            let adapter = Arc::new(DockerRemoteAdapter::new(ssh_config.clone()));
            adapter.connect().await
                .context("Falha ao conectar ao servidor Docker remoto")?;

            self.context = DockerExecutionContext::Remote {
                adapter,
                server_info: server_info.clone(),
            };

            self.context_config = DockerContextConfig {
                server_id: server_info.id.clone(),
                server_name: server_info.name.clone(),
                is_remote: true,
                ssh_config: Some(ssh_config),
            };
        }

        // Limpa estatísticas anteriores ao trocar contexto
        self.previous_stats.clear();
        Ok(())
    }

    /// Obtém informações do contexto atual
    pub fn get_context_config(&self) -> &DockerContextConfig {
        &self.context_config
    }

    /// Verifica se está em contexto remoto
    pub fn is_remote(&self) -> bool {
        self.context_config.is_remote
    }

    /// Obtém referência ao gerenciador de servidores
    pub fn get_server_manager(&self) -> &Arc<RemoteServerManager> {
        &self.server_manager
    }

    /// Cria instância com fallback inteligente (remoto -> local)
    pub async fn new_with_fallback(server_id: Option<String>, server_manager: Arc<RemoteServerManager>) -> Result<Self> {
        if let Some(id) = server_id {
            // Tenta conectar ao servidor remoto especificado
            if let Some(server_info) = server_manager.get_server(&id).await {
                match Self::try_remote_connection(&server_info, server_manager.clone()).await {
                    Ok(manager) => return Ok(manager),
                    Err(e) => {
                        eprintln!("Falha ao conectar servidor remoto '{}': {}. Usando local.", server_info.name, e);
                    }
                }
            }
        }

        // Fallback para Docker local
        Self::new_with_server_manager(server_manager).await
    }

    /// Tenta criar conexão remota
    async fn try_remote_connection(server_info: &ServerInfo, server_manager: Arc<RemoteServerManager>) -> Result<Self> {
        match &server_info.server_type {
            ServerType::Remote(ssh_config) => {
                let adapter = Arc::new(DockerRemoteAdapter::new(ssh_config.clone()));
                
                // Testa conexão
                adapter.connect().await.context("Falha na conexão SSH")?;
                
                Ok(DockerManager {
                    context: DockerExecutionContext::Remote {
                        adapter: adapter.clone(),
                        server_info: server_info.clone(),
                    },
                    context_config: DockerContextConfig {
                        server_id: server_info.id.clone(),
                        server_name: server_info.name.clone(),
                        is_remote: true,
                        ssh_config: Some(ssh_config.clone()),
                    },
                    previous_stats: HashMap::new(),
                    server_manager,
                })
            }
            ServerType::Local => {
                // Servidor "remoto" configurado como local
                Self::new().await
            }
        }
    }

    /// Obtém informações do contexto atual
    pub fn get_current_context(&self) -> &DockerContextConfig {
        &self.context_config
    }

    /// Verifica se está usando conexão remota
    pub fn is_using_remote(&self) -> bool {
        matches!(self.context, DockerExecutionContext::Remote { .. })
    }

    /// Obtém nome do servidor atual
    pub fn get_current_server_name(&self) -> &str {
        &self.context_config.server_name
    }

    /// Obtém ID do servidor atual
    pub fn get_current_server_id(&self) -> &str {
        &self.context_config.server_id
    }

    /// Testa conectividade do contexto atual
    pub async fn test_connectivity(&self) -> Result<bool> {
        match &self.context {
            DockerExecutionContext::Local(docker) => {
                match docker.ping().await {
                    Ok(_) => Ok(true),
                    Err(_) => Ok(false),
                }
            }
            DockerExecutionContext::Remote { adapter, .. } => {
                Ok(adapter.is_connected().await)
            }
        }
    }

    /// Reconecta ao contexto atual (útil para recuperar conexões perdidas)
    pub async fn reconnect(&mut self) -> Result<()> {
        match &self.context {
            DockerExecutionContext::Local(_) => {
                // Para conexões locais, recria a conexão
                let docker = Docker::connect_with_socket_defaults()
                    .context("Falha ao reconectar com Docker local")?;
                self.context = DockerExecutionContext::Local(docker);
                Ok(())
            }
            DockerExecutionContext::Remote { adapter, .. } => {
                adapter.connect().await.context("Falha ao reconectar SSH")
            }
        }
    }

    // Verifica se Docker daemon está respondendo
    // pub async fn is_docker_running(&self) -> Result<bool> {
    //     match self.docker.ping().await {
    //         Ok(_) => Ok(true),
    //         Err(err) => {
    //             println!("{}", err);
    //             Ok(false)
    //         }
    //     }
    // }

    /// Verifica status do Docker (local ou remoto)
    pub async fn check_docker_status(&self) -> DockerStatus {
        match &self.context {
            DockerExecutionContext::Local(_) => {
                // Para contexto local, usa verificação via linha de comando
                let docker_version = Command::new("docker").arg("--version").output();

                match docker_version {
                    Ok(output) => {
                        if !output.status.success() {
                            return DockerStatus::NotInstalled;
                        }
                    }
                    Err(_) => {
                        return DockerStatus::NotInstalled;
                    }
                }

                let docker_info = Command::new("docker").arg("info").output();

                match docker_info {
                    Ok(output) => {
                        if output.status.success() {
                            DockerStatus::Running
                        } else {
                            let stderr = String::from_utf8_lossy(&output.stderr);
                            let stdout = String::from_utf8_lossy(&output.stdout);

                            if stderr.contains("permission denied")
                                || stderr.contains("Permission denied")
                                || stderr.contains("dial unix")
                                || stderr.contains("connect: permission denied")
                                || stderr.contains("Got permission denied while trying to connect")
                                || stdout.contains("permission denied")
                            {
                                DockerStatus::PermissionDenied
                            } else if stderr.contains("Cannot connect to the Docker daemon")
                                || stderr.contains("Is the docker daemon running?")
                                || stderr.contains("docker daemon is not running")
                            {
                                DockerStatus::NotRunning
                            } else {
                                DockerStatus::PermissionDenied
                            }
                        }
                    }
                    Err(_) => DockerStatus::PermissionDenied,
                }
            }
            DockerExecutionContext::Remote { adapter, .. } => {
                // Para contexto remoto, testa conectividade SSH e Docker
                match adapter.test_connection().await {
                    Ok(true) => DockerStatus::Running,
                    Ok(false) => DockerStatus::NotRunning,
                    Err(_) => DockerStatus::NotRunning,
                }
            }
        }
    }

    /// Obtém informações gerais do Docker (local ou remoto)
    pub async fn get_docker_info(&self) -> Result<DockerInfo> {
        match &self.context {
            DockerExecutionContext::Local(docker) => {
                let version = docker
                    .version()
                    .await
                    .context("Falha ao obter versão do Docker")?;

                let info = docker
                    .info()
                    .await
                    .context("Falha ao obter informações do Docker")?;

                Ok(DockerInfo {
                    version: version.version.unwrap_or_default(),
                    containers: info.containers.unwrap_or(0),
                    containers_paused: info.containers_paused.unwrap_or(0),
                    containers_running: info.containers_running.unwrap_or(0),
                    containers_stopped: info.containers_stopped.unwrap_or(0),
                    images: info.images.unwrap_or(0),
                    architecture: version.arch.unwrap_or_default(),
                })
            }
            DockerExecutionContext::Remote { adapter, .. } => {
                // Para contexto remoto, usa informações do sistema
                if let Some(system_info) = adapter.get_system_info().await {
                    Ok(DockerInfo {
                        version: system_info.version,
                        containers: (system_info.containers_running + system_info.containers_paused + system_info.containers_stopped) as i64,
                        containers_paused: system_info.containers_paused as i64,
                        containers_running: system_info.containers_running as i64,
                        containers_stopped: system_info.containers_stopped as i64,
                        images: system_info.images as i64,
                        architecture: system_info.architecture,
                    })
                } else {
                    // Fallback com valores padrão se não conseguir obter informações
                    Ok(DockerInfo {
                        version: "Desconhecido".to_string(),
                        containers: 0,
                        containers_paused: 0,
                        containers_running: 0,
                        containers_stopped: 0,
                        images: 0,
                        architecture: "x86_64".to_string(),
                    })
                }
            }
        }
    }

    /// Lista todos os containers (ativos e parados)
    pub async fn list_containers(&self) -> Result<Vec<ContainerInfo>> {
        match &self.context {
            DockerExecutionContext::Local(docker) => {
                let containers = docker
                    .list_containers(Some(ListContainersOptions {
                        all: true,
                        ..Default::default()
                    }))
                    .await
                    .context("Falha ao listar containers")?;

                let container_infos: Vec<ContainerInfo> = containers
                    .into_iter()
                    .map(|container| ContainerInfo {
                        id: container.id.unwrap_or_default(),
                        name: container
                            .names
                            .unwrap_or_default()
                            .join(", ")
                            .trim_start_matches('/')
                            .to_string(),
                        image: container.image.unwrap_or_default(),
                        state: container
                            .state
                            .map_or("unknown".to_string(), |s| s.to_string()),
                        status: container.status.unwrap_or_default(),
                        ports: container
                            .ports
                            .unwrap_or_default()
                            .iter()
                            .filter_map(|port| port.public_port.map(|p| p as i32))
                            .collect(),
                        created: container.created.unwrap_or_default(),
                    })
                    .collect();

                Ok(container_infos)
            }
            DockerExecutionContext::Remote { adapter, .. } => {
                // Para contexto remoto, usa o adapter
                adapter.list_containers().await
                    .context("Falha ao listar containers remotos")
            }
        }
    }

    /// Inicia um container (local ou remoto)
    pub async fn start_container(&self, container_name: &str) -> Result<()> {
        match &self.context {
            DockerExecutionContext::Local(_) => {
                let output = Command::new("docker")
                    .args(&["start", container_name])
                    .output()
                    .context("Failed to execute docker start command")?;

                if !output.status.success() {
                    return Err(anyhow::anyhow!(
                        "Failed to start container {}: {}",
                        container_name,
                        String::from_utf8_lossy(&output.stderr)
                    ));
                }

                Ok(())
            }
            DockerExecutionContext::Remote { adapter, .. } => {
                adapter.start_container(container_name).await
                    .context("Falha ao iniciar container remoto")
            }
        }
    }

    /// Lista todas as imagens (local ou remoto)
    pub async fn list_images(&self) -> Result<Vec<ImageInfo>> {
        match &self.context {
            DockerExecutionContext::Local(docker) => {
                let images = docker
                    .list_images(Some(ListImagesOptions {
                        all: false,
                        ..Default::default()
                    }))
                    .await
                    .context("Falha ao listar imagens")?;

                let mut image_infos: Vec<ImageInfo> = images
                    .into_iter()
                    .map(|image: ImageSummary| {
                        let in_use = image.containers > 0;
                        ImageInfo {
                            id: image.id.clone(),
                            tags: image.repo_tags.clone(),
                            created: image.created,
                            size: image.size,
                            in_use,
                        }
                    })
                    .collect();

                // Ordena por nome da primeira tag para manter ordem consistente
                image_infos.sort_by(|a, b| {
                    let tag_a = a.tags.get(0).cloned().unwrap_or_default();
                    let tag_b = b.tags.get(0).cloned().unwrap_or_default();
                    tag_a.cmp(&tag_b)
                });

                Ok(image_infos)
            }
            DockerExecutionContext::Remote { adapter, .. } => {
                // Para contexto remoto, usa o adapter
                adapter.list_images().await
                    .context("Falha ao listar imagens remotas")
            }
        }
    }

    // deleta uma imagem
    /// Remove uma imagem (local ou remoto)
    pub async fn remove_image(&self, image_id: &str) -> Result<()> {
        match &self.context {
            DockerExecutionContext::Local(_) => {
                let output = Command::new("docker")
                    .args(&["rmi", image_id])
                    .output()
                    .context("Failed to execute docker rmi command")?;

                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr).to_lowercase();
                    if stderr.contains("conflict")
                        && stderr.contains("image is being used by running container")
                    {
                        return Err(anyhow::anyhow!(
                            "IN_USE:A imagem está em uso por um contêiner."
                        ));
                    } else {
                        return Err(anyhow::anyhow!(
                            "OTHER_ERROR:Não foi possível remover a imagem. Tente forçar a remoção."
                        ));
                    }
                }

                Ok(())
            }
            DockerExecutionContext::Remote { adapter, .. } => {
                adapter.remove_image(image_id, false).await
                    .context("Falha ao remover imagem remota")
            }
        }
    }

    /// Lista todas as networks (local ou remoto)
    pub async fn list_networks(&self) -> Result<Vec<NetworkInfo>> {
        match &self.context {
            DockerExecutionContext::Local(docker) => {
                let networks = docker
                    .list_networks(Some(ListNetworksOptions {
                        ..Default::default()
                    }))
                    .await
                    .context("Falha ao listar networks")?;

                let containers = docker
                    .list_containers(Some(ListContainersOptions {
                        all: true,
                        ..Default::default()
                    }))
                    .await
                    .context("Falha ao listar containers")?;

                let network_ids: Vec<String> = containers
                    .into_iter()
                    .flat_map(|container| {
                        container
                            .network_settings
                            .and_then(|settings| settings.networks)
                            .unwrap_or_default()
                            .into_values()
                            .filter_map(|endpoint| endpoint.network_id)
                    })
                    .collect();

                let mut network_infos: Vec<NetworkInfo> = networks
                    .into_iter()
                    .filter_map(|network| {
                        let network_name = network.name.as_deref().unwrap_or("");
                        let id = network.id.unwrap_or_default();

                        // Filtra networks de sistema (bridge, host, none)
                        let is_system = matches!(network_name, "bridge" | "host" | "none");

                        if is_system {
                            return None; // Pula networks de sistema
                        }

                        let mut containers_count = 0;

                        for network_id in &network_ids {
                            if network_id == &id {
                                containers_count += 1;
                            }
                        }

                        Some(NetworkInfo {
                            id,
                            name: network_name.to_string(),
                            driver: network.driver.unwrap_or_default(),
                            scope: network.scope.unwrap_or_default(),
                            created: network.created.unwrap_or_default(),
                            containers_count,
                            is_system: false, // Todas as networks listadas são de usuário
                        })
                    })
                    .collect();

                // Ordena por nome para manter ordem consistente
                network_infos.sort_by(|a, b| a.name.cmp(&b.name));

                Ok(network_infos)
            }
            DockerExecutionContext::Remote { adapter, .. } => {
                // Para contexto remoto, usa o adapter
                adapter.list_networks().await
                    .context("Falha ao listar networks remotas")
            }
        }
    }

    // Remove uma network
    pub async fn remove_network(&self, network_id: &str) -> Result<()> {
        let output = Command::new("docker")
            .args(&["network", "rm", network_id])
            .output()
            .context("Failed to execute docker network rm command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_lowercase();
            if stderr.contains("has active endpoints") || stderr.contains("endpoint") {
                return Err(anyhow::anyhow!(
                    "IN_USE:A network possui containers conectados."
                ));
            } else if stderr.contains("not found") || stderr.contains("no such network") {
                return Err(anyhow::anyhow!("OTHER_ERROR:Network não encontrada."));
            } else {
                return Err(anyhow::anyhow!(
                    "OTHER_ERROR:Não foi possível remover a network: {}",
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
        }

        Ok(())
    }

    // Lista todos os volumes de containers
    /// Lista todos os volumes (local ou remoto)
    pub async fn list_volumes(&self) -> Result<Vec<VolumeInfo>> {
        match &self.context {
            DockerExecutionContext::Local(docker) => {
                let volumes = docker
                    .list_volumes(Some(ListVolumesOptions {
                        ..Default::default()
                    }))
                    .await
                    .context("Falha ao listar volumes")?;

                let containers = docker
                    .list_containers(Some(ListContainersOptions {
                        all: true,
                        ..Default::default()
                    }))
                    .await
                    .context("Falha ao listar containers")?;

                // Coleta nomes de volumes usados pelos containers
                let mut used_volumes: HashMap<String, i32> = HashMap::new();
                for container in containers {
                    if let Some(mounts) = container.mounts {
                        for mount in mounts {
                            if let Some(name) = mount.name {
                                if let Some(mount_type) = mount.typ.as_ref() {
                                    if format!("{:?}", mount_type)
                                        .to_lowercase()
                                        .contains("volume")
                                    {
                                        *used_volumes.entry(name).or_insert(0) += 1;
                                    }
                                }
                            }
                        }
                    }
                }

                let mut volume_infos: Vec<VolumeInfo> = volumes
                    .volumes
                    .unwrap_or_default()
                    .into_iter()
                    .map(|volume| {
                        let volume_name = volume.name.clone();
                        // Agora inclui TODOS os volumes, mas com contador de containers
                        let containers_count = used_volumes.get(&volume_name).cloned().unwrap_or(0);

                        VolumeInfo {
                            name: volume_name,
                            driver: volume.driver,
                            mountpoint: volume.mountpoint,
                            created: volume.created_at.unwrap_or_default(),
                            containers_count,
                        }
                    })
                    .collect();

                // Ordena por nome para manter ordem consistente
                volume_infos.sort_by(|a, b| a.name.cmp(&b.name));

                Ok(volume_infos)
            }
            DockerExecutionContext::Remote { adapter, .. } => {
                // Para contexto remoto, usa o adapter
                adapter.list_volumes().await
                    .context("Falha ao listar volumes remotos")
            }
        }
    }

    // Remove um volume
    pub async fn remove_volume(&self, volume_name: &str) -> Result<()> {
        let output = Command::new("docker")
            .args(&["volume", "rm", volume_name])
            .output()
            .context("Failed to execute docker volume rm command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_lowercase();
            if stderr.contains("volume is in use") || stderr.contains("in use") {
                return Err(anyhow::anyhow!(
                    "IN_USE:O volume está sendo usado por containers."
                ));
            } else if stderr.contains("not found") || stderr.contains("no such volume") {
                return Err(anyhow::anyhow!("OTHER_ERROR:Volume não encontrado."));
            } else {
                return Err(anyhow::anyhow!(
                    "OTHER_ERROR:Não foi possível remover o volume: {}",
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
        }

        Ok(())
    }

    // Para um container
    /// Para um container (local ou remoto)
    pub async fn stop_container(&self, container_name: &str) -> Result<()> {
        match &self.context {
            DockerExecutionContext::Local(_) => {
                let output = Command::new("docker")
                    .args(&["stop", container_name])
                    .output()
                    .context("Failed to execute docker stop command")?;

                if !output.status.success() {
                    return Err(anyhow::anyhow!(
                        "Failed to stop container {}: {}",
                        container_name,
                        String::from_utf8_lossy(&output.stderr)
                    ));
                }

                Ok(())
            }
            DockerExecutionContext::Remote { adapter, .. } => {
                adapter.stop_container(container_name).await
                    .context("Falha ao parar container remoto")
            }
        }
    }

    // Pausa um container
    pub async fn pause_container(&self, container_name: &str) -> Result<()> {
        let output = Command::new("docker")
            .args(&["pause", container_name])
            .output()
            .context("Failed to execute docker pause command")?;

        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "Failed to pause container {}: {}",
                container_name,
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        Ok(())
    }

    // Despausa um container
    pub async fn unpause_container(&self, container_name: &str) -> Result<()> {
        let output = Command::new("docker")
            .args(&["unpause", container_name])
            .output()
            .context("Failed to execute docker unpause command")?;

        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "Failed to unpause container {}: {}",
                container_name,
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        Ok(())
    }

    // deleta um container
    pub async fn remove_container(&self, container_name: &str) -> Result<()> {
        let output = Command::new("docker")
            .args(&["rm", container_name])
            .output()
            .context("Failed to execute docker unpause command")?;

        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "Failed to unpause container {}: {}",
                container_name,
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        Ok(())
    }

    /// Lista apenas containers em execução (local ou remoto)
    pub async fn list_running_containers(&self) -> Result<Vec<ContainerInfo>> {
        match &self.context {
            DockerExecutionContext::Local(docker) => {
                let containers = docker
                    .list_containers(Some(ListContainersOptions {
                        all: false, // Apenas containers rodando
                        ..Default::default()
                    }))
                    .await
                    .context("Falha ao listar containers ativos")?;

                let container_infos: Vec<ContainerInfo> = containers
                    .into_iter()
                    .map(|container| ContainerInfo {
                        id: container.id.unwrap_or_default(),
                        name: container
                            .names
                            .unwrap_or_default()
                            .join(", ")
                            .trim_start_matches('/')
                            .to_string(),
                        image: container.image.unwrap_or_default(),
                        state: container
                            .state
                            .map_or("unknown".to_string(), |s| s.to_string()),
                        status: container.status.unwrap_or_default(),
                        ports: container
                            .ports
                            .unwrap_or_default()
                            .iter()
                            .filter_map(|port| port.public_port.map(|p| p as i32))
                            .collect(),
                        created: container.created.unwrap_or_default(),
                    })
                    .collect();

                Ok(container_infos)
            }
            DockerExecutionContext::Remote { adapter, .. } => {
                // Para remoto, filtra apenas containers em execução
                let all_containers = adapter.list_containers().await
                    .context("Falha ao listar containers remotos ativos")?;
                
                let running_containers = all_containers
                    .into_iter()
                    .filter(|container| container.state == "running")
                    .collect();
                
                Ok(running_containers)
            }
        }
    }

    /// Coleta uso total do sistema Docker (local ou remoto)
    pub async fn get_docker_system_usage(&mut self) -> Result<DockerSystemUsage> {
        let containers = self.list_running_containers().await?;
        let mut containers_stats = Vec::new();

        // Totalizadores de recursos
        let mut total_cpu = 0.0;
        let mut online_cpu = 0;
        let mut total_memory_usage = 0u64;
        let total_memory_limit = self.get_system_memory_limit().await?;
        let mut total_network_rx = 0u64;
        let mut total_network_tx = 0u64;
        let mut total_block_read = 0u64;
        let mut total_block_write = 0u64;

        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Itera por todos os containers coletando estatísticas
        for container in containers {
            match &self.context {
                DockerExecutionContext::Local(docker) => {
                    if let Ok(Some(stats)) = docker
                        .stats(
                            &container.id,
                            Some(StatsOptions {
                                stream: false,
                                one_shot: true,
                            }),
                        )
                        .try_next()
                        .await
                    {
                        let cpu =
                            self.calculate_cpu_percentage_with_cache(&container.id, &stats, current_time);
                        let cpu_percentage = cpu.usage_cpu;
                        let cpus_online = cpu.online_cpus;
                        let memory_usage = stats
                            .memory_stats
                            .as_ref()
                            .and_then(|m| m.usage)
                            .unwrap_or(0);
                        let memory_limit = stats
                            .memory_stats
                            .as_ref()
                            .and_then(|m| m.limit)
                            .unwrap_or(0);

                        let memory_percentage = if memory_limit > 0 {
                            (memory_usage as f64 / memory_limit as f64) * 100.0
                        } else {
                            0.0
                        };

                        let (network_rx, network_tx) = self.get_network_stats(&stats);
                        let (block_read, block_write) = self.get_block_stats(&stats);

                        containers_stats.push(ContainerStats {
                            id: container.id.clone(),
                            name: container.name.clone(),
                            cpu_percentage,
                            memory_usage,
                            memory_limit,
                            memory_percentage,
                            network_rx,
                            network_tx,
                            block_read,
                            block_write,
                        });

                        online_cpu = cpus_online;
                        total_cpu += cpu_percentage;
                        total_memory_usage += memory_usage;
                        total_network_rx += network_rx;
                        total_network_tx += network_tx;
                        total_block_read += block_read;
                        total_block_write += block_write;
                    }
                }
                DockerExecutionContext::Remote { adapter, .. } => {
                    // Para remoto, obtém estatísticas via SSH
                    if let Ok(_stats_json) = adapter.get_container_stats(&container.id).await {
                        // Placeholder - parsing de estatísticas remotas
                        // Em implementação real, faria parsing do JSON retornado
                        
                        // Adiciona estatísticas básicas como placeholder
                        containers_stats.push(ContainerStats {
                            id: container.id.clone(),
                            name: container.name.clone(),
                            cpu_percentage: 0.0,
                            memory_usage: 0,
                            memory_limit: 0,
                            memory_percentage: 0.0,
                            network_rx: 0,
                            network_tx: 0,
                            block_read: 0,
                            block_write: 0,
                        });
                        
                        online_cpu = 1; // Placeholder
                    }
                }
            }
        }

        let memory_percentage = if total_memory_limit > 0 {
            (total_memory_usage as f64 / total_memory_limit as f64) * 100.0
        } else {
            0.0
        };

        Ok(DockerSystemUsage {
            cpu_online: online_cpu,
            cpu_usage: total_cpu,
            memory_usage: total_memory_usage,
            memory_limit: total_memory_limit,
            memory_percentage,
            network_rx_bytes: total_network_rx,
            network_tx_bytes: total_network_tx,
            block_read_bytes: total_block_read,
            block_write_bytes: total_block_write,
            containers_stats,
        })
    }

    // Calcula CPU com cache de estatísticas anteriores
    fn calculate_cpu_percentage_with_cache(
        &mut self,
        container_id: &str,
        stats: &ContainerStatsResponse,
        current_time: u64,
    ) -> CpuCalculate {
        if let (Some(cpu_stats), Some(precpu_stats)) = (&stats.cpu_stats, &stats.precpu_stats) {
            if let (Some(cpu_usage), Some(precpu_usage)) = (
                cpu_stats.cpu_usage.as_ref(),
                precpu_stats.cpu_usage.as_ref(),
            ) {
                let cpu_total = cpu_usage.total_usage.unwrap_or(0);
                let system_total = cpu_stats.system_cpu_usage.unwrap_or(0);

                // Verifica se temos estatísticas anteriores para este container
                let cpu_delta;
                let system_delta;

                if let Some(prev_stats) = self.previous_stats.get(container_id) {
                    // Usa dados do cache interno, mas verifica se são válidos
                    let cache_cpu_delta = cpu_total.saturating_sub(prev_stats.cpu_total);
                    let cache_system_delta = system_total.saturating_sub(prev_stats.system_total);

                    // Se o delta do cache é muito pequeno, usa precpu_stats como fallback
                    if cache_system_delta > 0 && cache_cpu_delta <= cache_system_delta * 10 {
                        cpu_delta = cache_cpu_delta;
                        system_delta = cache_system_delta;
                    } else {
                        // Fallback para precpu_stats se cache não parece confiável
                        let cpu_total_prev = precpu_usage.total_usage.unwrap_or(0);
                        let system_total_prev = precpu_stats.system_cpu_usage.unwrap_or(0);
                        cpu_delta = cpu_total.saturating_sub(cpu_total_prev);
                        system_delta = system_total.saturating_sub(system_total_prev);
                    }
                } else {
                    // Primeira vez - usa precpu_stats
                    let cpu_total_prev = precpu_usage.total_usage.unwrap_or(0);
                    let system_total_prev = precpu_stats.system_cpu_usage.unwrap_or(0);

                    cpu_delta = cpu_total.saturating_sub(cpu_total_prev);
                    system_delta = system_total.saturating_sub(system_total_prev);
                }

                // Atualiza cache para próxima iteração
                let (network_rx, network_tx) = self.get_network_stats(stats);
                let (block_read, block_write) = self.get_block_stats(stats);

                // Número de CPUs online (preferido) ou número de CPUs por core
                let number_cpus = if let Some(online_cpus) = cpu_stats.online_cpus {
                    online_cpus as f64
                } else {
                    // Fallback: conta CPUs disponíveis por percpu_usage
                    if let Some(percpu_usage) = &cpu_usage.percpu_usage {
                        // Conta apenas CPUs que não são zero (ativas)
                        percpu_usage
                            .iter()
                            .filter(|&&usage| usage > 0)
                            .count()
                            .max(1) as f64
                    } else {
                        1.0 // Fallback mínimo
                    }
                };

                self.previous_stats.insert(
                    container_id.to_string(),
                    PreviousStats {
                        timestamp: current_time,
                        cpu_total,
                        system_total,
                        network_rx,
                        network_tx,
                        block_read,
                        block_write,
                    },
                );

                // Evita divisão por zero - só retorna 0 se system_delta for 0
                if system_delta == 0 {
                    return CpuCalculate {
                        online_cpus: number_cpus as u64,
                        usage_cpu: 0.0,
                    };
                }

                // Se cpu_delta for 0, significa que não houve uso de CPU nesse período
                if cpu_delta == 0 {
                    return CpuCalculate {
                        online_cpus: number_cpus as u64,
                        usage_cpu: 0.0,
                    };
                }

                // Fórmula correta do Docker CLI
                let cpu_percent = (cpu_delta as f64 / system_delta as f64) * number_cpus * 100.0;

                // Debug para valores anômalos apenas se necessário
                if cpu_percent > number_cpus * 100.0 {
                    eprintln!(
                        "DEBUG CPU [{}]: cpu_delta={}, system_delta={}, cpus={}, percent={:.2}%",
                        &container_id[..12],
                        cpu_delta,
                        system_delta,
                        number_cpus,
                        cpu_percent
                    );
                    // Para valores muito altos, pode haver problema nos dados
                    return CpuCalculate {
                        online_cpus: 0,
                        usage_cpu: 0.0,
                    };
                }

                // Limita resultado a um valor razoável

                CpuCalculate {
                    online_cpus: number_cpus as u64,
                    usage_cpu: cpu_percent.max(0.0).min(100.0 * number_cpus),
                }
            } else {
                CpuCalculate {
                    online_cpus: 0,
                    usage_cpu: 0.0,
                }
            }
        } else {
            CpuCalculate {
                online_cpus: 0,
                usage_cpu: 0.0,
            }
        }
    }

    // Método para limpar cache antigo (executar periodicamente)
    // pub fn cleanup_old_stats(&mut self, max_age_seconds: u64) {
    //     let current_time = SystemTime::now()
    //         .duration_since(UNIX_EPOCH)
    //         .unwrap()
    //         .as_secs();

    //     self.previous_stats
    //         .retain(|_, stats| current_time - stats.timestamp < max_age_seconds);
    // }

    // Obtém estatísticas de rede (RX/TX)
    fn get_network_stats(&self, stats: &ContainerStatsResponse) -> (u64, u64) {
        if let Some(networks) = &stats.networks {
            let mut rx_bytes = 0u64;
            let mut tx_bytes = 0u64;

            // Soma dados de todas as interfaces de rede
            for (_, network) in networks {
                rx_bytes += network.rx_bytes.unwrap_or(0);
                tx_bytes += network.tx_bytes.unwrap_or(0);
            }

            (rx_bytes, tx_bytes)
        } else {
            (0, 0)
        }
    }

    // Obtém estatísticas de I/O de disco
    fn get_block_stats(&self, stats: &ContainerStatsResponse) -> (u64, u64) {
        if let Some(blkio_stats) = &stats.blkio_stats {
            let mut read_bytes = 0u64;
            let mut write_bytes = 0u64;

            // Soma operações de leitura e escrita em disco
            if let Some(io_service_bytes_recursive) = &blkio_stats.io_service_bytes_recursive {
                for entry in io_service_bytes_recursive {
                    if let Some(op) = &entry.op {
                        match op.as_str() {
                            "Read" => read_bytes += entry.value.unwrap_or(0),
                            "Write" => write_bytes += entry.value.unwrap_or(0),
                            _ => {}
                        }
                    }
                }
            }

            (read_bytes, write_bytes)
        } else {
            (0, 0)
        }
    }

    // Função auxiliar para obter limite de memória do sistema
    async fn get_system_memory_limit(&self) -> Result<u64> {
        match &self.context {
            DockerExecutionContext::Local(docker) => {
                match docker.info().await {
                    Ok(info) => {
                        // Tenta obter memória total do sistema via Docker info
                        Ok(info.mem_total.unwrap_or(0) as u64)
                    }
                    Err(_) => {
                        // Fallback: lê do sistema de arquivos Linux
                        self.get_system_memory_from_meminfo()
                    }
                }
            }
            DockerExecutionContext::Remote { .. } => {
                // Para conexões remotas, use fallback
                self.get_system_memory_from_meminfo()
            }
        }
    }

    // Função para ler memória do sistema via /proc/meminfo (Linux)
    fn get_system_memory_from_meminfo(&self) -> Result<u64> {
        use std::fs;

        match fs::read_to_string("/proc/meminfo") {
            Ok(content) => {
                for line in content.lines() {
                    if line.starts_with("MemTotal:") {
                        if let Some(mem_str) = line.split_whitespace().nth(1) {
                            if let Ok(mem_kb) = mem_str.parse::<u64>() {
                                return Ok(mem_kb * 1024); // Converte KB para bytes
                            }
                        }
                    }
                }
                Ok(0) // Se não conseguir encontrar, retorna 0
            }
            Err(_) => {
                // Se não conseguir ler /proc/meminfo, usa um valor padrão
                // ou retorna erro
                Ok(8_589_934_592) // 8GB como fallback
            }
        }
    }

    // pub async fn print_containers(&self) -> Result<()> {
    //     let containers = self.list_containers().await?;

    //     if containers.is_empty() {
    //         println!("📭 Nenhum container encontrado");
    //         return Ok(());
    //     }

    //     println!("📦 Containers encontrados: {}\n", containers.len());

    //     for (i, container) in containers.iter().enumerate() {
    //         println!("Container #{}", i + 1);
    //         println!(
    //             "  🆔 ID: {}",
    //             &container.id[..std::cmp::min(12, container.id.len())]
    //         );
    //         println!("  📛 Nome: {}", container.name);
    //         println!("  🖼️  Imagem: {}", container.image);

    //         let emoji = match container.state.as_str() {
    //             "running" => "🟢",
    //             "exited" => "🔴",
    //             "paused" => "⏸️",
    //             "created" => "🟡",
    //             _ => "⚪",
    //         };
    //         println!("  {} Estado: {}", emoji, container.state);
    //         println!("  📊 Status: {}", container.status);
    //         println!();
    //     }

    //     Ok(())
    // }

    // pub async fn start_container(&self, container_id: &str) -> Result<()> {
    //     self.docker
    //         .start_container::<String>(container_id, None)
    //         .await
    //         .context(format!("Falha ao iniciar container: {}", container_id))?;

    //     println!("✅ Container {} iniciado com sucesso!", container_id);
    //     Ok(())
    // }

    // pub async fn stop_container(&self, container_id: &str) -> Result<()> {
    //     self.docker
    //         .stop_container(container_id, None)
    //         .await
    //         .context(format!("Falha ao parar container: {}", container_id))?;

    //     println!("🛑 Container {} parado com sucesso!", container_id);
    //     Ok(())
    // }

    /// Reinicia um container (local ou remoto)
    pub async fn restart_container(&self, container_id: &str) -> Result<()> {
        match &self.context {
            DockerExecutionContext::Local(docker) => {
                docker
                    .restart_container(container_id, None::<RestartContainerOptions>)
                    .await
                    .context(format!("Falha ao reiniciar container: {}", container_id))?;
                Ok(())
            }
            DockerExecutionContext::Remote { adapter, .. } => {
                adapter.restart_container(container_id).await
                    .context("Falha ao reiniciar container remoto")
            }
        }
    }

    // Obter logs de um container com paginação
    /// Obtém logs de container (local ou remoto)
    pub async fn get_container_logs(
        &self,
        container_name: &str,
        tail_lines: Option<String>,
    ) -> Result<String> {
        match &self.context {
            DockerExecutionContext::Local(docker) => {
                use bollard::query_parameters::LogsOptions;
                use futures_util::StreamExt;

                let logs_options = LogsOptions {
                    stdout: true,
                    stderr: true,
                    tail: tail_lines.unwrap_or_else(|| "50".to_string()), // Padrão: últimas 50 linhas
                    timestamps: true,
                    ..Default::default()
                };

                let mut logs_stream = docker.logs(container_name, Some(logs_options));

                let mut logs = String::new();
                while let Some(log_result) = logs_stream.next().await {
                    match log_result {
                        Ok(log_output) => {
                            logs.push_str(&log_output.to_string());
                        }
                        Err(_) => break,
                    }
                }

                // Processa e formata os logs com timestamp
                let formatted_logs = logs
                    .lines()
                    .filter_map(|line| {
                        if line.len() > 30 {
                            // Tenta extrair o timestamp (formato: 2023-01-01T00:00:00.000000000Z)
                            let timestamp_str = &line[0..30];
                            let message = if line.len() > 31 { &line[31..] } else { "" };

                            // Parse do timestamp ISO 8601 usando chrono
                            if let Ok(utc_time) = timestamp_str.parse::<chrono::DateTime<chrono::Utc>>() {
                                let local_time = utc_time.with_timezone(&chrono::Local);
                                let formatted_time = local_time.format("%H:%M:%S").to_string();
                                Some(format!("[{}] {}", formatted_time, message))
                            } else {
                                // Se não conseguir parsear timestamp, retorna a linha original sem timestamp
                                Some(message.to_string())
                            }
                        } else {
                            // Linha muito curta, provavelmente não tem timestamp
                            Some(line.to_string())
                        }
                    })
                    .collect::<Vec<String>>()
                    .join("\n");

                Ok(if formatted_logs.trim().is_empty() {
                    "Nenhum log disponível".to_string()
                } else {
                    formatted_logs
                })
            }
            DockerExecutionContext::Remote { adapter, .. } => {
                // Para remoto, usa adapter SSH
                let lines_count = tail_lines
                    .and_then(|s| s.parse::<usize>().ok());
                
                adapter.get_container_logs(container_name, lines_count).await
                    .context("Falha ao obter logs de container remoto")
            }
        }
    }

    // Obter logs anteriores de um container (para infinite scroll)
    // pub async fn get_container_logs_before(
    //     &self,
    //     container_name: &str,
    //     before_timestamp: &str,
    //     lines: u32,
    // ) -> Result<String> {
    //     use bollard::query_parameters::LogsOptions;
    //     use futures_util::StreamExt;

    //     let logs_options = LogsOptions {
    //         stdout: true,
    //         stderr: true,
    //         until: before_timestamp.parse::<i32>().unwrap_or(0), // Logs antes deste timestamp
    //         tail: lines.to_string(),
    //         timestamps: true,
    //         ..Default::default()
    //     };

    //     let mut logs_stream = self.docker.logs(container_name, Some(logs_options));

    //     let mut logs = String::new();
    //     while let Some(log_result) = logs_stream.next().await {
    //         match log_result {
    //             Ok(log_output) => {
    //                 logs.push_str(&log_output.to_string());
    //             }
    //             Err(_) => break,
    //         }
    //     }

    //     // Processa e formata os logs com timestamp
    //     let formatted_logs = logs
    //         .lines()
    //         .filter_map(|line| {
    //             if line.len() > 30 {
    //                 let timestamp_str = &line[0..30];
    //                 let message = if line.len() > 31 { &line[31..] } else { "" };

    //                 if let Ok(utc_time) = timestamp_str.parse::<chrono::DateTime<chrono::Utc>>() {
    //                     let local_time = utc_time.with_timezone(&chrono::Local);
    //                     let formatted_time = local_time.format("%H:%M:%S").to_string();
    //                     Some(format!("[{}] {}", formatted_time, message))
    //                 } else {
    //                     Some(message.to_string())
    //                 }
    //             } else {
    //                 Some(line.to_string())
    //             }
    //         })
    //         .collect::<Vec<String>>()
    //         .join("\n");

    //     Ok(formatted_logs)
    // }

    // Obter estatísticas de um container específico
    /// Obtém estatísticas de um container específico (local ou remoto)
    pub async fn get_single_container_stats(
        &mut self,
        container_name: &str,
    ) -> Result<(f64, u64, String, String, String)> {
        match &self.context {
            DockerExecutionContext::Local(docker) => {
                use bollard::query_parameters::StatsOptions;
                use futures_util::StreamExt;

                let stats_options = Some(StatsOptions {
                    stream: false,
                    one_shot: true,
                });

                let mut stats_stream = docker.stats(container_name, stats_options);

                if let Some(stats_result) = stats_stream.next().await {
                    match stats_result {
                        Ok(stats) => {
                            // Calcula CPU usando função existente
                            let current_time = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_secs();
                            let cpu_calc = self.calculate_cpu_percentage_with_cache(
                                container_name,
                                &stats,
                                current_time,
                            );
                            let cpu_usage = cpu_calc.usage_cpu;
                            let cpu_online = cpu_calc.online_cpus;

                            // Calcula memória
                            let memory_stats = stats.memory_stats.as_ref().cloned().unwrap_or_default();
                            let memory_usage = memory_stats.usage.unwrap_or(0);
                            let memory_limit = memory_stats.limit.unwrap_or(0);

                            let memory_usage_mb = memory_usage as f64 / 1024.0 / 1024.0;
                            let memory_limit_mb = memory_limit as f64 / 1024.0 / 1024.0;
                            let memory_percentage = if memory_limit > 0 {
                                (memory_usage as f64 / memory_limit as f64) * 100.0
                            } else {
                                0.0
                            };

                            let memory_str = if memory_usage_mb >= 1024.0 || memory_limit_mb >= 1024.0 {
                                let memory_usage_gb = memory_usage_mb / 1024.0;
                                let memory_limit_gb = memory_limit_mb / 1024.0;

                                if memory_usage_mb >= 1024.0 && memory_limit_mb >= 1024.0 {
                                    format!(
                                        "{:.1}% ({:.1} GB / {:.1} GB)",
                                        memory_percentage, memory_usage_gb, memory_limit_gb
                                    )
                                } else if memory_usage_mb >= 1024.0 {
                                    format!(
                                        "{:.1}% ({:.1} GB / {:.0} MB)",
                                        memory_percentage, memory_usage_gb, memory_limit_mb
                                    )
                                } else {
                                    format!(
                                        "{:.1}% ({:.0} MB / {:.1} GB)",
                                        memory_percentage, memory_usage_mb, memory_limit_gb
                                    )
                                }
                            } else {
                                format!(
                                    "{:.1}% ({:.0} MB / {:.0} MB)",
                                    memory_percentage, memory_usage_mb, memory_limit_mb
                                )
                            };

                            // Calcula network
                            let (rx, tx) = self.get_network_stats(&stats);
                            let rx_str = self.format_bytes_rate(rx);
                            let tx_str = self.format_bytes_rate(tx);

                            Ok((cpu_usage, cpu_online, memory_str, rx_str, tx_str))
                        }
                        Err(e) => Err(anyhow::anyhow!("Erro ao obter stats do container: {}", e)),
                    }
                } else {
                    Err(anyhow::anyhow!("Nenhum stat recebido para o container"))
                }
            }
            DockerExecutionContext::Remote { adapter, .. } => {
                // Para remoto, usa adapter SSH
                match adapter.get_container_stats(container_name).await {
                    Ok(_stats_json) => {
                        // Placeholder - parsing de estatísticas remotas
                        // Em implementação real, faria parsing do JSON e cálculos similares
                        Ok((
                            0.0, // CPU usage placeholder
                            1,   // CPU online placeholder
                            "0.0% (0 MB / 0 MB)".to_string(), // Memory placeholder
                            "0 B/s".to_string(), // RX placeholder
                            "0 B/s".to_string(), // TX placeholder
                        ))
                    }
                    Err(e) => Err(anyhow::anyhow!("Erro ao obter stats do container remoto: {}", e)),
                }
            }
        }
    }

    // Função auxiliar para formatar bytes/s
    fn format_bytes_rate(&self, bytes: u64) -> String {
        if bytes < 1024 {
            format!("{} B/s", bytes)
        } else if bytes < 1024 * 1024 {
            format!("{:.1} KB/s", bytes as f64 / 1024.0)
        } else {
            format!("{:.1} MB/s", bytes as f64 / 1024.0 / 1024.0)
        }
    }

    // Cria um novo container
    pub async fn create_container(&self, request: CreateContainerRequest) -> Result<String> {
        use bollard::models::{
            HostConfig, Mount, MountTypeEnum, PortBinding, RestartPolicy, RestartPolicyNameEnum,
        };
        use std::collections::HashMap;

        // Verifica se o nome já existe
        if self.container_name_exists(&request.name).await? {
            return Err(anyhow::anyhow!(
                "Container com nome '{}' já existe",
                request.name
            ));
        }

        // Verifica se a imagem existe localmente, se não, tenta fazer pull
        if !self.image_exists(&request.image).await? {
            // Aqui podemos adicionar callback de progresso no futuro
            self.pull_image(&request.image).await?;
        }

        // Configura mapeamento de portas
        let mut port_bindings: HashMap<String, Option<Vec<PortBinding>>> = HashMap::new();
        let mut exposed_ports: HashMap<String, HashMap<(), ()>> = HashMap::new();

        for port_map in &request.ports {
            let container_port_key = format!("{}/{}", port_map.container_port, port_map.protocol);
            port_bindings.insert(
                container_port_key.clone(),
                Some(vec![PortBinding {
                    host_ip: Some("0.0.0.0".to_string()),
                    host_port: Some(port_map.host_port.to_string()),
                }]),
            );
            exposed_ports.insert(container_port_key, HashMap::new());
        }

        // Configura volumes/mounts
        let mut mounts = Vec::new();
        for volume_map in &request.volumes {
            mounts.push(Mount {
                target: Some(volume_map.container_path.clone()),
                source: Some(volume_map.host_path.clone()),
                typ: Some(MountTypeEnum::BIND),
                read_only: Some(volume_map.read_only),
                ..Default::default()
            });
        }

        // Configura variáveis de ambiente
        let env: Vec<String> = request
            .environment
            .iter()
            .map(|var| format!("{}={}", var.key, var.value))
            .collect();

        // Configura política de restart
        let restart_policy = match request.restart_policy.as_str() {
            "always" => Some(RestartPolicy {
                name: Some(RestartPolicyNameEnum::ALWAYS),
                maximum_retry_count: None,
            }),
            "unless-stopped" => Some(RestartPolicy {
                name: Some(RestartPolicyNameEnum::UNLESS_STOPPED),
                maximum_retry_count: None,
            }),
            "on-failure" => Some(RestartPolicy {
                name: Some(RestartPolicyNameEnum::ON_FAILURE),
                maximum_retry_count: Some(3),
            }),
            _ => Some(RestartPolicy {
                name: Some(RestartPolicyNameEnum::EMPTY),
                maximum_retry_count: None,
            }),
        };

        // Configura comando se especificado
        let cmd = request.command.as_ref().map(|c| {
            c.split_whitespace()
                .map(|s| s.to_string())
                .collect::<Vec<String>>()
        });

        // Cria configuração do container
        let config = ContainerCreateBody {
            image: Some(request.image.clone()),
            env: Some(env),
            cmd,
            exposed_ports: Some(exposed_ports),
            host_config: Some(HostConfig {
                port_bindings: Some(port_bindings),
                mounts: Some(mounts),
                restart_policy,
                ..Default::default()
            }),
            ..Default::default()
        };

        let options = CreateContainerOptions {
            name: Some(request.name.clone()),
            ..Default::default()
        };

        // Cria o container
        let response = match &self.context {
            DockerExecutionContext::Local(docker) => {
                docker
                    .create_container(Some(options), config)
                    .await
                    .context("Falha ao criar container")?
            }
            DockerExecutionContext::Remote { adapter, .. } => {
                return Err(anyhow::anyhow!("Criação de container remoto não implementada"));
            }
        };

        // Inicia o container automaticamente
        self.start_container(&response.id)
            .await
            .context("Container criado mas falha ao iniciar")?;

        Ok(response.id)
    }

    // Verifica se um container com o nome existe
    async fn container_name_exists(&self, name: &str) -> Result<bool> {
        let containers = self.list_containers().await?;
        Ok(containers.iter().any(|c| c.name == name))
    }

    // Verifica se uma imagem existe localmente
    async fn image_exists(&self, image_name: &str) -> Result<bool> {
        let images = self.list_images().await?;
        Ok(images.iter().any(|img| {
            img.tags
                .iter()
                .any(|tag| tag == image_name || tag.starts_with(&format!("{}:", image_name)))
        }))
    }

    // Faz pull de uma imagem
    async fn pull_image(&self, image_name: &str) -> Result<()> {
        use bollard::query_parameters::CreateImageOptions;
        use futures_util::StreamExt;

        let options = CreateImageOptions {
            from_image: Some(image_name.to_string()),
            ..Default::default()
        };

        let mut stream = match &self.context {
            DockerExecutionContext::Local(docker) => {
                docker.create_image(Some(options), None, None)
            }
            DockerExecutionContext::Remote { .. } => {
                return Err(anyhow::anyhow!("Download de imagem remoto não implementado"));
            }
        };

        while let Some(result) = stream.next().await {
            match result {
                Ok(_) => {
                    // Pull em progresso
                }
                Err(e) => {
                    return Err(anyhow::anyhow!(
                        "Falha ao fazer pull da imagem '{}': {}",
                        image_name,
                        e
                    ));
                }
            }
        }

        Ok(())
    }
}
