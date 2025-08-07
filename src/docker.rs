// Imports para gerenciamento do Docker
use anyhow::{Context, Result};
use bollard::{
    Docker,
    models::ContainerStatsResponse,
    query_parameters::{ListContainersOptions, StatsOptions},
};
use futures_util::TryStreamExt;
use serde::{Deserialize, Serialize};
use std::{fmt, process::Command};

// Informações básicas de um container
#[derive(Debug, Serialize, Deserialize)]
pub struct ContainerInfo {
    pub id: String,
    pub name: String,
    pub image: String,
    pub state: String,
    pub status: String,
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

// Gerenciador principal do Docker
pub struct DockerManager {
    docker: Docker,
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

// Uso total do sistema Docker
#[derive(Debug, Serialize, Deserialize)]
pub struct DockerSystemUsage {
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

impl DockerManager {
    // Cria nova instância conectando ao Docker daemon
    pub async fn new() -> Result<Self> {
        let docker = Docker::connect_with_socket_defaults()
            .context("Falha ao conectar com Docker daemon")?;

        Ok(DockerManager { docker })
    }

    // Verifica se Docker daemon está respondendo
    pub async fn is_docker_running(&self) -> Result<bool> {
        match self.docker.ping().await {
            Ok(_) => Ok(true),
            Err(err) => {
                println!("{}", err);
                Ok(false)
            }
        }
    }

    // Verifica status do Docker via linha de comando
    pub fn check_docker_status(&self) -> DockerStatus {
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

    // Obtém informações gerais do Docker
    pub async fn get_docker_info(&self) -> Result<DockerInfo> {
        let version = self
            .docker
            .version()
            .await
            .context("Falha ao obter versão do Docker")?;

        let info = self
            .docker
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

    // Lista todos os containers (ativos e parados)
    pub async fn list_containers(&self) -> Result<Vec<ContainerInfo>> {
        let containers = self
            .docker
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
            })
            .collect();

        Ok(container_infos)
    }

    // Lista apenas containers em execução
    pub async fn list_running_containers(&self) -> Result<Vec<ContainerInfo>> {
        let containers = self
            .docker
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
            })
            .collect();

        Ok(container_infos)
    }

    // Coleta uso total do sistema Docker
    pub async fn get_docker_system_usage(&self) -> Result<DockerSystemUsage> {
        let containers = self.list_running_containers().await?;
        let mut containers_stats = Vec::new();
        // Totalizadores de recursos
        let mut total_cpu = 0.0;
        let mut total_memory_usage = 0u64;
        let total_memory_limit = self.get_system_memory_limit().await?;
        let mut total_network_rx = 0u64;
        let mut total_network_tx = 0u64;
        let mut total_block_read = 0u64;
        let mut total_block_write = 0u64;
        // Itera por todos os containers coletando estatísticas

        for container in containers {
            if let Ok(Some(stats)) = self
                .docker
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
                let cpu_percentage = self.calculate_cpu_percentage(&stats);
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

                total_cpu += cpu_percentage;
                total_memory_usage += memory_usage;
                total_network_rx += network_rx;
                total_network_tx += network_tx;
                total_block_read += block_read;
                total_block_write += block_write;
            }
        }

        let memory_percentage = if total_memory_limit > 0 {
            (total_memory_usage as f64 / total_memory_limit as f64) * 100.0
        } else {
            0.0
        };

        Ok(DockerSystemUsage {
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

    // Calcula porcentagem de uso de CPU
    fn calculate_cpu_percentage(&self, stats: &ContainerStatsResponse) -> f64 {
        if let (Some(cpu_stats), Some(precpu_stats)) = (&stats.cpu_stats, &stats.precpu_stats) {
            if let (Some(cpu_usage), Some(precpu_usage)) = (
                cpu_stats.cpu_usage.as_ref(),
                precpu_stats.cpu_usage.as_ref(),
            ) {
                let cpu_total = cpu_usage.total_usage.unwrap_or(0);
                let cpu_total_prev = precpu_usage.total_usage.unwrap_or(0);
                let system_total = cpu_stats.system_cpu_usage.unwrap_or(0);
                let system_total_prev = precpu_stats.system_cpu_usage.unwrap_or(0);

                let cpu_delta = cpu_total.saturating_sub(cpu_total_prev);
                let system_delta = system_total.saturating_sub(system_total_prev);

                // Evita divisão por zero e valores muito pequenos
                if system_delta == 0 || cpu_delta == 0 {
                    return 0.0;
                }

                let number_cpus = cpu_stats.online_cpus.unwrap_or(1) as f64;

                // Algoritmo correto do Docker CLI
                let cpu_percent = (cpu_delta as f64 / system_delta as f64) * number_cpus * 100.0;

                // CORREÇÃO: Limita a 100% POR CONTAINER, não por sistema
                let result = cpu_percent.min(100.0);

                // Se ainda estiver muito alto, há um problema com os dados
                if result > 100.0 {
                    println!(
                        "WARNING: CPU > 100% - cpu_delta: {}, system_delta: {}, cpus: {}",
                        cpu_delta, system_delta, number_cpus
                    );
                    return 0.0; // Retorna 0 se dados parecem inválidos
                }

                return result;
            }
        }
        0.0
    }

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
        match self.docker.info().await {
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

    // pub async fn restart_container(&self, container_id: &str) -> Result<()> {
    //     self.docker
    //         .restart_container(container_id, None)
    //         .await
    //         .context(format!("Falha ao reiniciar container: {}", container_id))?;

    //     println!("🔄 Container {} reiniciado com sucesso!", container_id);
    //     Ok(())
    // }
}
