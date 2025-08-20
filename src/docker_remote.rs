use crate::docker::{
    ContainerInfo, ContainerStats, CreateContainerRequest, DockerInfo, DockerStatus,
    DockerSystemUsage, ImageInfo, NetworkInfo, VolumeInfo,
};
use crate::ssh::{SshClient, SshConnection};
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde_json;
use std::{collections::HashMap, time::{SystemTime, UNIX_EPOCH}};

#[async_trait]
pub trait DockerManager {
    async fn list_containers(&self) -> Result<Vec<ContainerInfo>>;
    async fn start_container(&self, container_name: &str) -> Result<()>;
    async fn stop_container(&self, container_name: &str) -> Result<()>;
    async fn restart_container(&self, container_name: &str) -> Result<()>;
    async fn remove_container(&self, container_name: &str) -> Result<()>;
    async fn pause_container(&self, container_name: &str) -> Result<()>;
    async fn unpause_container(&self, container_name: &str) -> Result<()>;
    async fn list_images(&self) -> Result<Vec<ImageInfo>>;
    async fn remove_image(&self, image_id: &str) -> Result<()>;
    async fn list_networks(&self) -> Result<Vec<NetworkInfo>>;
    async fn remove_network(&self, network_id: &str) -> Result<()>;
    async fn list_volumes(&self) -> Result<Vec<VolumeInfo>>;
    async fn remove_volume(&self, volume_name: &str) -> Result<()>;
    async fn get_docker_info(&self) -> Result<DockerInfo>;
    async fn get_container_logs(
        &self,
        container_name: &str,
        lines: Option<usize>,
    ) -> Result<String>;
    fn check_docker_status(&self) -> DockerStatus;
    async fn list_running_containers(&self) -> Result<Vec<ContainerInfo>>;
    async fn get_docker_system_usage(&mut self) -> Result<DockerSystemUsage>;
    async fn get_single_container_stats(
        &mut self,
        container_name: &str,
    ) -> Result<(f64, u64, String, String, String)>;
    async fn create_container(&self, request: CreateContainerRequest) -> Result<String>;
}

// Cache para estatísticas anteriores (necessário para cálculo de delta) 
#[derive(Debug, Clone)]
struct PreviousStats {
    timestamp: u64,
    cpu_total: u64,
    system_total: u64,
    network_rx: u64,
    network_tx: u64,
    block_read: u64,
    block_write: u64,
}

pub struct RemoteDockerManager {
    ssh_client: SshClient,
    connected: bool,
    previous_stats: HashMap<String, PreviousStats>,
}

impl RemoteDockerManager {
    pub fn new() -> Self {
        Self {
            ssh_client: SshClient::new(),
            connected: false,
            previous_stats: HashMap::new(),
        }
    }

    pub async fn connect(&mut self, connection: SshConnection) -> Result<()> {
        self.ssh_client
            .connect(connection)
            .await
            .context("Failed to connect via SSH")?;
        self.connected = true;

        // Verificar se Docker está instalado no servidor remoto
        let result = self.ssh_client.execute_command("docker --version").await?;
        if result.exit_code != 0 {
            return Err(anyhow::anyhow!("Docker not found on remote server"));
        }

        Ok(())
    }

    pub fn is_connected(&self) -> bool {
        self.connected && self.ssh_client.is_connected()
    }

    pub fn disconnect(&mut self) {
        self.ssh_client.disconnect();
        self.connected = false;
    }

    async fn execute_docker_command(&self, command: &str) -> Result<String> {
        if !self.is_connected() {
            return Err(anyhow::anyhow!("Not connected to remote server"));
        }

        let full_command = format!("docker {}", command);
        let result = self.ssh_client.execute_command(&full_command).await?;

        if result.exit_code != 0 {
            return Err(anyhow::anyhow!("Docker command failed: {}", result.stderr));
        }

        Ok(result.stdout)
    }

    async fn parse_containers_from_json(&self, json_output: &str) -> Result<Vec<ContainerInfo>> {
        let mut containers = Vec::new();

        for line in json_output.lines() {
            if line.trim().is_empty() {
                continue;
            }

            match serde_json::from_str::<serde_json::Value>(line) {
                Ok(container) => {
                    let id = container["Id"].as_str().unwrap_or("").to_string();
                    let names = container["Names"]
                        .as_array()
                        .and_then(|arr| arr.first())
                        .and_then(|name| name.as_str())
                        .unwrap_or("")
                        .trim_start_matches('/')
                        .to_string();

                    let image = container["Image"].as_str().unwrap_or("").to_string();
                    let state = container["State"].as_str().unwrap_or("").to_string();
                    let status = container["Status"].as_str().unwrap_or("").to_string();
                    let created = container["Created"].as_i64().unwrap_or(0);

                    // Parse ports
                    let mut ports = Vec::new();
                    if let Some(ports_array) = container["Ports"].as_array() {
                        for port in ports_array {
                            if let Some(public_port) = port["PublicPort"].as_i64() {
                                ports.push(public_port as i32);
                            }
                        }
                    }

                    containers.push(ContainerInfo {
                        id,
                        name: names,
                        image,
                        state,
                        status,
                        ports,
                        created,
                    });
                }
                Err(e) => eprintln!("Failed to parse container JSON: {}", e),
            }
        }

        Ok(containers)
    }

    async fn parse_images_from_json(&self, json_output: &str) -> Result<Vec<ImageInfo>> {
        let mut images = Vec::new();

        for line in json_output.lines() {
            if line.trim().is_empty() {
                continue;
            }

            match serde_json::from_str::<serde_json::Value>(line) {
                Ok(image) => {
                    let id = image["Id"].as_str().unwrap_or("").to_string();
                    let repo_tags = image["RepoTags"]
                        .as_array()
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|tag| tag.as_str())
                                .map(|s| s.to_string())
                                .collect()
                        })
                        .unwrap_or_else(|| vec!["<none>".to_string()]);

                    let created = image["Created"].as_i64().unwrap_or(0);
                    let size = image["Size"].as_i64().unwrap_or(0);

                    images.push(ImageInfo {
                        id,
                        tags: repo_tags,
                        created,
                        size,
                        in_use: false, // Seria necessário verificar separadamente
                    });
                }
                Err(e) => eprintln!("Failed to parse image JSON: {}", e),
            }
        }

        Ok(images)
    }

    // Implementações adicionais seguindo o padrão do docker.rs

    pub fn check_docker_status(&self) -> DockerStatus {
        if !self.is_connected() {
            return DockerStatus::NotRunning;
        }
        // Para SSH remoto, assumimos que se conectou, está rodando
        DockerStatus::Running
    }

    pub async fn list_running_containers(&self) -> Result<Vec<ContainerInfo>> {
        let output = self.execute_docker_command("ps --format json").await?;
        self.parse_containers_from_json(&output).await
    }

    pub async fn pause_container(&self, container_name: &str) -> Result<()> {
        self.execute_docker_command(&format!("pause {}", container_name)).await?;
        Ok(())
    }

    pub async fn unpause_container(&self, container_name: &str) -> Result<()> {
        self.execute_docker_command(&format!("unpause {}", container_name)).await?;
        Ok(())
    }

    pub async fn remove_network(&self, network_id: &str) -> Result<()> {
        let output = self.execute_docker_command(&format!("network rm {}", network_id)).await;
        
        match output {
            Ok(_) => Ok(()),
            Err(e) => {
                let error_msg = e.to_string().to_lowercase();
                if error_msg.contains("has active endpoints") || error_msg.contains("endpoint") {
                    Err(anyhow::anyhow!("IN_USE:A network possui containers conectados."))
                } else if error_msg.contains("not found") || error_msg.contains("no such network") {
                    Err(anyhow::anyhow!("OTHER_ERROR:Network não encontrada."))
                } else {
                    Err(anyhow::anyhow!("OTHER_ERROR:Não foi possível remover a network: {}", e))
                }
            }
        }
    }

    pub async fn remove_volume(&self, volume_name: &str) -> Result<()> {
        let output = self.execute_docker_command(&format!("volume rm {}", volume_name)).await;
        
        match output {
            Ok(_) => Ok(()),
            Err(e) => {
                let error_msg = e.to_string().to_lowercase();
                if error_msg.contains("volume is in use") || error_msg.contains("in use") {
                    Err(anyhow::anyhow!("IN_USE:O volume está sendo usado por containers."))
                } else if error_msg.contains("not found") || error_msg.contains("no such volume") {
                    Err(anyhow::anyhow!("OTHER_ERROR:Volume não encontrado."))
                } else {
                    Err(anyhow::anyhow!("OTHER_ERROR:Não foi possível remover o volume: {}", e))
                }
            }
        }
    }

    pub async fn get_docker_system_usage(&mut self) -> Result<DockerSystemUsage> {
        let containers = self.list_running_containers().await?;
        let mut containers_stats = Vec::new();

        let mut total_cpu = 0.0;
        let mut online_cpu = 0;
        let mut total_memory_usage = 0u64;
        let total_memory_limit = self.get_system_memory_limit().await?;
        let mut total_network_rx = 0u64;
        let mut total_network_tx = 0u64;
        let mut total_block_read = 0u64;
        let mut total_block_write = 0u64;

        let _current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        for container in containers {
            if let Ok((cpu_percentage, cpu_online_val, memory_usage, memory_limit, network_rx, network_tx, block_read, block_write)) = 
                self.get_container_stats_raw(&container.id).await {
                
                let memory_percentage = if memory_limit > 0 {
                    (memory_usage as f64 / memory_limit as f64) * 100.0
                } else {
                    0.0
                };

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

                online_cpu = cpu_online_val;
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

    pub async fn get_single_container_stats(
        &mut self,
        container_name: &str,
    ) -> Result<(f64, u64, String, String, String)> {
        let (cpu_percentage, cpu_online, memory_usage, memory_limit, network_rx, network_tx, _, _) = 
            self.get_container_stats_raw(container_name).await?;

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

        let rx_str = self.format_bytes_rate(network_rx);
        let tx_str = self.format_bytes_rate(network_tx);

        Ok((cpu_percentage, cpu_online, memory_str, rx_str, tx_str))
    }

    pub async fn create_container(&self, request: CreateContainerRequest) -> Result<String> {
        // Verifica se o nome já existe
        if self.container_name_exists(&request.name).await? {
            return Err(anyhow::anyhow!(
                "Container com nome '{}' já existe",
                request.name
            ));
        }

        // Constrói o comando docker run
        let mut cmd_parts = vec!["run".to_string(), "-d".to_string()];
        
        // Adiciona nome
        cmd_parts.push("--name".to_string());
        cmd_parts.push(request.name.clone());

        // Adiciona portas
        for port_map in &request.ports {
            cmd_parts.push("-p".to_string());
            cmd_parts.push(format!(
                "{}:{}/{}",
                port_map.host_port, port_map.container_port, port_map.protocol
            ));
        }

        // Adiciona volumes
        for volume_map in &request.volumes {
            let mount_type = if volume_map.read_only { "ro" } else { "rw" };
            cmd_parts.push("-v".to_string());
            cmd_parts.push(format!(
                "{}:{}:{}",
                volume_map.host_path, volume_map.container_path, mount_type
            ));
        }

        // Adiciona variáveis de ambiente
        for env_var in &request.environment {
            cmd_parts.push("-e".to_string());
            cmd_parts.push(format!("{}={}", env_var.key, env_var.value));
        }

        // Adiciona política de restart
        match request.restart_policy.as_str() {
            "always" => {
                cmd_parts.push("--restart".to_string());
                cmd_parts.push("always".to_string());
            }
            "unless-stopped" => {
                cmd_parts.push("--restart".to_string());
                cmd_parts.push("unless-stopped".to_string());
            }
            "on-failure" => {
                cmd_parts.push("--restart".to_string());
                cmd_parts.push("on-failure:3".to_string());
            }
            _ => {}
        }

        // Adiciona imagem
        cmd_parts.push(request.image.clone());

        // Adiciona comando se especificado
        if let Some(command) = &request.command {
            cmd_parts.extend(command.split_whitespace().map(|s| s.to_string()));
        }

        let full_command = cmd_parts.join(" ");
        let output = self.execute_docker_command(&full_command).await?;
        
        // O comando docker run retorna o ID do container
        let container_id = output.trim().to_string();
        Ok(container_id)
    }

    // Funções auxiliares
    async fn get_container_stats_raw(
        &self,
        container_id: &str,
    ) -> Result<(f64, u64, u64, u64, u64, u64, u64, u64)> {
        let output = self
            .execute_docker_command(&format!("stats --no-stream --format json {}", container_id))
            .await?;

        match serde_json::from_str::<serde_json::Value>(&output.trim()) {
            Ok(stats) => {
                let cpu_percent = stats["CPUPerc"]
                    .as_str()
                    .unwrap_or("0%")
                    .trim_end_matches('%')
                    .parse::<f64>()
                    .unwrap_or(0.0);

                let memory_usage_str = stats["MemUsage"].as_str().unwrap_or("0B / 0B");
                let (memory_usage, memory_limit) = self.parse_memory_usage(memory_usage_str);

                let net_io_str = stats["NetIO"].as_str().unwrap_or("0B / 0B");
                let (network_rx, network_tx) = self.parse_network_io(net_io_str);

                let block_io_str = stats["BlockIO"].as_str().unwrap_or("0B / 0B");
                let (block_read, block_write) = self.parse_block_io(block_io_str);

                let cpu_online = self.get_cpu_count().await.unwrap_or(1);

                Ok((
                    cpu_percent,
                    cpu_online,
                    memory_usage,
                    memory_limit,
                    network_rx,
                    network_tx,
                    block_read,
                    block_write,
                ))
            }
            Err(e) => Err(anyhow::anyhow!("Failed to parse container stats: {}", e)),
        }
    }

    async fn get_system_memory_limit(&self) -> Result<u64> {
        match self.execute_docker_command("info --format '{{.MemTotal}}'").await {
            Ok(output) => {
                let mem_str = output.trim();
                Ok(mem_str.parse::<u64>().unwrap_or(0))
            }
            Err(_) => {
                // Fallback: tenta obter via comando do sistema
                match self.ssh_client.execute_command("cat /proc/meminfo | grep MemTotal").await {
                    Ok(result) => {
                        for line in result.stdout.lines() {
                            if line.starts_with("MemTotal:") {
                                if let Some(mem_str) = line.split_whitespace().nth(1) {
                                    if let Ok(mem_kb) = mem_str.parse::<u64>() {
                                        return Ok(mem_kb * 1024); // Converte KB para bytes
                                    }
                                }
                            }
                        }
                        Ok(8_589_934_592) // 8GB como fallback
                    }
                    Err(_) => Ok(8_589_934_592), // 8GB como fallback
                }
            }
        }
    }

    async fn get_cpu_count(&self) -> Result<u64> {
        match self.ssh_client.execute_command("nproc").await {
            Ok(result) => {
                let cpu_count = result.stdout.trim().parse::<u64>().unwrap_or(1);
                Ok(cpu_count)
            }
            Err(_) => Ok(1), // Fallback
        }
    }

    fn parse_memory_usage(&self, mem_usage_str: &str) -> (u64, u64) {
        let parts: Vec<&str> = mem_usage_str.split(" / ").collect();
        if parts.len() == 2 {
            let usage = self.parse_bytes(parts[0].trim());
            let limit = self.parse_bytes(parts[1].trim());
            (usage, limit)
        } else {
            (0, 0)
        }
    }

    fn parse_network_io(&self, net_io_str: &str) -> (u64, u64) {
        let parts: Vec<&str> = net_io_str.split(" / ").collect();
        if parts.len() == 2 {
            let rx = self.parse_bytes(parts[0].trim());
            let tx = self.parse_bytes(parts[1].trim());
            (rx, tx)
        } else {
            (0, 0)
        }
    }

    fn parse_block_io(&self, block_io_str: &str) -> (u64, u64) {
        let parts: Vec<&str> = block_io_str.split(" / ").collect();
        if parts.len() == 2 {
            let read = self.parse_bytes(parts[0].trim());
            let write = self.parse_bytes(parts[1].trim());
            (read, write)
        } else {
            (0, 0)
        }
    }

    fn parse_bytes(&self, bytes_str: &str) -> u64 {
        if bytes_str.is_empty() {
            return 0;
        }

        let bytes_str = bytes_str.to_lowercase();
        let (number_part, unit) = if bytes_str.ends_with("kb") {
            (&bytes_str[..bytes_str.len() - 2], 1024_u64)
        } else if bytes_str.ends_with("mb") {
            (&bytes_str[..bytes_str.len() - 2], 1024_u64.pow(2))
        } else if bytes_str.ends_with("gb") {
            (&bytes_str[..bytes_str.len() - 2], 1024_u64.pow(3))
        } else if bytes_str.ends_with("tb") {
            (&bytes_str[..bytes_str.len() - 2], 1024_u64.pow(4))
        } else if bytes_str.ends_with("b") {
            (&bytes_str[..bytes_str.len() - 1], 1)
        } else {
            (bytes_str.as_str(), 1)
        };

        number_part.parse::<f64>().unwrap_or(0.0) as u64 * unit
    }

    fn format_bytes_rate(&self, bytes: u64) -> String {
        if bytes < 1024 {
            format!("{} B/s", bytes)
        } else if bytes < 1024 * 1024 {
            format!("{:.1} KB/s", bytes as f64 / 1024.0)
        } else {
            format!("{:.1} MB/s", bytes as f64 / 1024.0 / 1024.0)
        }
    }

    async fn container_name_exists(&self, name: &str) -> Result<bool> {
        let containers = self.list_containers().await?;
        Ok(containers.iter().any(|c| c.name == name))
    }
}

#[async_trait]
impl DockerManager for RemoteDockerManager {
    async fn list_containers(&self) -> Result<Vec<ContainerInfo>> {
        let output = self.execute_docker_command("ps -a --format json").await?;
        self.parse_containers_from_json(&output).await
    }

    async fn start_container(&self, container_name: &str) -> Result<()> {
        self.execute_docker_command(&format!("start {}", container_name))
            .await?;
        Ok(())
    }

    async fn stop_container(&self, container_name: &str) -> Result<()> {
        self.execute_docker_command(&format!("stop {}", container_name))
            .await?;
        Ok(())
    }

    async fn restart_container(&self, container_name: &str) -> Result<()> {
        self.execute_docker_command(&format!("restart {}", container_name))
            .await?;
        Ok(())
    }

    async fn remove_container(&self, container_name: &str) -> Result<()> {
        self.execute_docker_command(&format!("rm -f {}", container_name))
            .await?;
        Ok(())
    }

    async fn pause_container(&self, container_name: &str) -> Result<()> {
        self.execute_docker_command(&format!("pause {}", container_name))
            .await?;
        Ok(())
    }

    async fn unpause_container(&self, container_name: &str) -> Result<()> {
        self.execute_docker_command(&format!("unpause {}", container_name))
            .await?;
        Ok(())
    }

    async fn list_images(&self) -> Result<Vec<ImageInfo>> {
        let output = self.execute_docker_command("images --format json").await?;
        self.parse_images_from_json(&output).await
    }

    async fn remove_image(&self, image_id: &str) -> Result<()> {
        self.execute_docker_command(&format!("rmi -f {}", image_id))
            .await?;
        Ok(())
    }

    async fn list_networks(&self) -> Result<Vec<NetworkInfo>> {
        let output = self
            .execute_docker_command("network ls --format json")
            .await?;
        let mut networks = Vec::new();

        for line in output.lines() {
            if line.trim().is_empty() {
                continue;
            }

            match serde_json::from_str::<serde_json::Value>(line) {
                Ok(network) => {
                    let id = network["ID"].as_str().unwrap_or("").to_string();
                    let name = network["Name"].as_str().unwrap_or("").to_string();
                    let driver = network["Driver"].as_str().unwrap_or("").to_string();
                    let scope = network["Scope"].as_str().unwrap_or("").to_string();
                    let is_system = name == "bridge" || name == "host" || name == "none";

                    networks.push(NetworkInfo {
                        id,
                        name,
                        driver,
                        scope,
                        created: "".to_string(),
                        containers_count: 0, // Seria necessário inspecionar a rede
                        is_system,
                    });
                }
                Err(e) => eprintln!("Failed to parse network JSON: {}", e),
            }
        }

        Ok(networks)
    }

    async fn list_volumes(&self) -> Result<Vec<VolumeInfo>> {
        let output = self
            .execute_docker_command("volume ls --format json")
            .await?;
        let mut volumes = Vec::new();

        for line in output.lines() {
            if line.trim().is_empty() {
                continue;
            }

            match serde_json::from_str::<serde_json::Value>(line) {
                Ok(volume) => {
                    let name = volume["Name"].as_str().unwrap_or("").to_string();
                    let driver = volume["Driver"].as_str().unwrap_or("").to_string();

                    volumes.push(VolumeInfo {
                        name,
                        driver,
                        mountpoint: "".to_string(), // Seria necessário inspecionar o volume
                        created: "".to_string(),
                        containers_count: 0,
                    });
                }
                Err(e) => eprintln!("Failed to parse volume JSON: {}", e),
            }
        }

        Ok(volumes)
    }

    async fn get_docker_info(&self) -> Result<DockerInfo> {
        let output = self.execute_docker_command("info --format json").await?;

        match serde_json::from_str::<serde_json::Value>(&output) {
            Ok(info) => {
                let containers_running = info["ContainersRunning"].as_i64().unwrap_or(0);
                let containers_stopped = info["ContainersStopped"].as_i64().unwrap_or(0);
                let containers_paused = info["ContainersPaused"].as_i64().unwrap_or(0);
                let images = info["Images"].as_i64().unwrap_or(0);
                let containers = info["Containers"].as_i64().unwrap_or(0);
                let version = info["ServerVersion"].as_str().unwrap_or("").to_string();
                let architecture = info["Architecture"].as_str().unwrap_or("").to_string();

                Ok(DockerInfo {
                    version,
                    containers,
                    containers_paused,
                    containers_running,
                    containers_stopped,
                    images,
                    architecture,
                })
            }
            Err(e) => Err(anyhow::anyhow!("Failed to parse Docker info: {}", e)),
        }
    }

    async fn get_container_logs(
        &self,
        container_name: &str,
        lines: Option<usize>,
    ) -> Result<String> {
        let lines_param = lines.map(|n| format!("--tail {}", n)).unwrap_or_default();
        let command = format!("logs {} {}", lines_param, container_name);
        self.execute_docker_command(&command).await
    }

    fn check_docker_status(&self) -> DockerStatus {
        if !self.is_connected() {
            return DockerStatus::NotRunning;
        }
        DockerStatus::Running
    }

    async fn list_running_containers(&self) -> Result<Vec<ContainerInfo>> {
        self.list_running_containers().await
    }

    async fn remove_network(&self, network_id: &str) -> Result<()> {
        self.remove_network(network_id).await
    }

    async fn remove_volume(&self, volume_name: &str) -> Result<()> {
        self.remove_volume(volume_name).await
    }

    async fn get_docker_system_usage(&mut self) -> Result<DockerSystemUsage> {
        self.get_docker_system_usage().await
    }

    async fn get_single_container_stats(
        &mut self,
        container_name: &str,
    ) -> Result<(f64, u64, String, String, String)> {
        self.get_single_container_stats(container_name).await
    }

    async fn create_container(&self, request: CreateContainerRequest) -> Result<String> {
        self.create_container(request).await
    }
}
