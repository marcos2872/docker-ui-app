pub mod types;

use crate::ssh::{SshClient, SshConnection};
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde_json;
use std::{
    collections::HashMap,
    time::{SystemTime, UNIX_EPOCH},
};
pub use types::{
    ContainerInfo, ContainerStats, CreateContainerRequest, DockerInfo, DockerStatus,
    DockerSystemUsage, ImageInfo, NetworkInfo, VolumeInfo,
};


#[async_trait]
pub trait DockerManagement {
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

pub struct DockerManager {
    ssh_client: SshClient,
    connected: bool,
    previous_stats: HashMap<String, PreviousStats>,
}

impl DockerManager {
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
                    .as_str()
                        .unwrap_or("")
                        .trim_start_matches('/')
                        .to_string();

                    let image = container["Image"].as_str().unwrap_or("").to_string();
                    let state = container["State"].as_str().unwrap_or("").to_string();
                    let status = container["Status"].as_str().unwrap_or("").to_string();
                    let created = container["Created"].as_i64().unwrap_or(0);
                    
                    // Parse ports string (formato completo como "8080:80/tcp, 443:443/tcp")
                    let ports_str = container["Ports"].as_str().unwrap_or("").to_string();
                    
                    // Parse running time
                    let running_for = container["RunningFor"].as_str().unwrap_or("").to_string();

                    // Parse ports list (mantendo compatibilidade)
                    let mut ports_list = Vec::new();
                    if let Some(ports_array) = container["Ports"].as_array() {
                        for port in ports_array {
                            if let Some(public_port) = port["PublicPort"].as_i64() {
                                ports_list.push(public_port as i32);
                            }
                        }
                    }

                    containers.push(ContainerInfo {
                        id,
                        name: names,
                        image,
                        state,
                        status,
                        ports: ports_str,
                        ports_list,
                        created,
                        running_for,
                    });
                }
                Err(_) => {}
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
                    let id = image["ID"].as_str().unwrap_or("").to_string();
                    
                    // Constrói nome da imagem como "repository:tag"
                    let repository = image["Repository"].as_str().unwrap_or("<none>");
                    let tag = image["Tag"].as_str().unwrap_or("<none>");
                    
                    let repo_tags = if repository == "<none>" && tag == "<none>" {
                        vec!["<none>:<none>".to_string()]
                    } else {
                        vec![format!("{}:{}", repository, tag)]
                    };

                    // Converte CreatedAt para timestamp Unix se possível
                    let created = if let Some(created_str) = image["CreatedAt"].as_str() {
                        // Tenta parsear a data, se falhar usa 0
                        chrono::DateTime::parse_from_str(created_str, "%Y-%m-%d %H:%M:%S %z")
                            .map(|dt| dt.timestamp())
                            .unwrap_or(0)
                    } else {
                        0
                    };
                    
                    // Converte tamanho de string para bytes
                    let size = Self::parse_size_string(image["VirtualSize"].as_str().unwrap_or("0B"));

                    images.push(ImageInfo {
                        id,
                        tags: repo_tags,
                        created,
                        size,
                        in_use: false, // Seria necessário verificar separadamente
                    });
                }
                Err(_) => {}
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
        self.execute_docker_command(&format!("pause {}", container_name))
            .await?;
        Ok(())
    }

    pub async fn unpause_container(&self, container_name: &str) -> Result<()> {
        self.execute_docker_command(&format!("unpause {}", container_name))
            .await?;
        Ok(())
    }

    pub async fn remove_network(&self, network_id: &str) -> Result<()> {
        // Primeiro verifica se a network está em uso
        let network_in_use = self.is_network_used_by_containers(network_id).await;
            
        if network_in_use {
            return Err(anyhow::anyhow!("Não é possível remover esta network pois ela possui containers conectados"));
        }

        let output = self
            .execute_docker_command(&format!("network rm {}", network_id))
            .await;

        match output {
            Ok(_) => Ok(()),
            Err(e) => {
                let error_msg = e.to_string().to_lowercase();
                if error_msg.contains("has active endpoints") || error_msg.contains("endpoint") {
                    Err(anyhow::anyhow!(
                        "IN_USE:A network possui containers conectados."
                    ))
                } else if error_msg.contains("not found") || error_msg.contains("no such network") {
                    Err(anyhow::anyhow!("OTHER_ERROR:Network não encontrada."))
                } else {
                    Err(anyhow::anyhow!(
                        "OTHER_ERROR:Não foi possível remover a network: {}",
                        e
                    ))
                }
            }
        }
    }

    pub async fn remove_volume(&self, volume_name: &str) -> Result<()> {
        // Primeiro verifica se o volume está em uso
        let volume_in_use = self.is_volume_used_by_containers(volume_name).await;
            
        if volume_in_use {
            return Err(anyhow::anyhow!("Não é possível remover este volume pois ele está sendo usado por containers"));
        }

        let output = self
            .execute_docker_command(&format!("volume rm {}", volume_name))
            .await;

        match output {
            Ok(_) => Ok(()),
            Err(e) => {
                let error_msg = e.to_string().to_lowercase();
                if error_msg.contains("volume is in use") || error_msg.contains("in use") {
                    Err(anyhow::anyhow!(
                        "IN_USE:O volume está sendo usado por containers."
                    ))
                } else if error_msg.contains("not found") || error_msg.contains("no such volume") {
                    Err(anyhow::anyhow!("OTHER_ERROR:Volume não encontrado."))
                } else {
                    Err(anyhow::anyhow!(
                        "OTHER_ERROR:Não foi possível remover o volume: {}",
                        e
                    ))
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
            let (
                cpu_percentage,
                cpu_online_val,
                memory_usage,
                memory_limit,
                network_rx,
                network_tx,
                block_read,
                block_write,
            ) = self.get_container_stats_raw(&container.id).await?;

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
            .execute_docker_command(&format!(
                "stats --no-stream --format \"{{{{json .}}}}\" {}",
                container_id
            ))
            .await?;

        match serde_json::from_str::<serde_json::Value>(&output.trim()) {
            Ok(stats) => {
            let cpu_str = stats["CPUPerc"].as_str().unwrap_or("0%");
            let cpu_percent = if cpu_str == "--" {
                0.0
            } else {
                cpu_str.trim_end_matches('%').parse::<f64>().unwrap_or(0.0)
            };

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
        match self
            .execute_docker_command("info --format '{{.MemTotal}}'")
            .await
        {
            Ok(output) => {
                let mem_str = output.trim();
                Ok(mem_str.parse::<u64>().unwrap_or(0))
            }
            Err(_) => {
                // Fallback: tenta obter via comando do sistema
                match self
                    .ssh_client
                    .execute_command("cat /proc/meminfo | grep MemTotal")
                    .await
                {
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
        let bytes_str = bytes_str.to_lowercase().trim().to_string();
        if bytes_str.is_empty() {
            return 0;
        }

        let mut number_part = String::new();
        let mut unit_part = String::new();

        for char in bytes_str.chars() {
            if char.is_digit(10) || char == '.' {
                number_part.push(char);
            } else {
                unit_part.push(char);
            }
        }

        let number = number_part.parse::<f64>().unwrap_or(0.0);
        let unit_part = unit_part.trim();

        let multiplier = match unit_part {
            "b" => 1.0,
            "kb" | "kib" => 1024.0,
            "mb" | "mib" => 1024.0 * 1024.0,
            "gb" | "gib" => 1024.0 * 1024.0 * 1024.0,
            "tb" | "tib" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
            _ => 1.0, // Assume bytes if no unit
        };

        (number * multiplier) as u64
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

    // Converte string de tamanho (ex: "137.1MB") para bytes
    fn parse_size_string(size_str: &str) -> i64 {
        if size_str.is_empty() || size_str == "0B" {
            return 0;
        }

        let size_str = size_str.trim();
        let mut multiplier = 1i64;
        let mut numeric_part = size_str;

        if size_str.ends_with("B") {
            numeric_part = &size_str[..size_str.len() - 1];
            
            if numeric_part.ends_with("K") {
                multiplier = 1024;
                numeric_part = &numeric_part[..numeric_part.len() - 1];
            } else if numeric_part.ends_with("M") {
                multiplier = 1024 * 1024;
                numeric_part = &numeric_part[..numeric_part.len() - 1];
            } else if numeric_part.ends_with("G") {
                multiplier = 1024 * 1024 * 1024;
                numeric_part = &numeric_part[..numeric_part.len() - 1];
            } else if numeric_part.ends_with("T") {
                multiplier = 1024 * 1024 * 1024 * 1024;
                numeric_part = &numeric_part[..numeric_part.len() - 1];
            }
        }

        numeric_part.parse::<f64>()
            .map(|num| (num * multiplier as f64) as i64)
            .unwrap_or(0)
    }
}

#[async_trait]
impl DockerManagement for DockerManager {
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
        let mut images = self.parse_images_from_json(&output).await?;
        
        // Verifica quais imagens estão em uso por containers
        let containers = self.list_containers().await?;
        
        for image in &mut images {
            image.in_use = containers.iter().any(|container| {
                // Verifica se algum container usa esta imagem com comparação flexível
                let container_image = &container.image;
                
                image.tags.iter().any(|tag| {
                    // Comparação exata
                    if container_image == tag {
                        return true;
                    }
                    
                    // Se container não tem tag, adiciona ":latest" 
                    let container_with_latest = if !container_image.contains(':') {
                        format!("{}:latest", container_image)
                    } else {
                        container_image.clone()
                    };
                    
                    // Se imagem não tem tag, adiciona ":latest"
                    let tag_with_latest = if !tag.contains(':') {
                        format!("{}:latest", tag)
                    } else {
                        tag.clone()
                    };
                    
                    // Compara com tags normalizadas
                    container_with_latest == *tag || container_image == &tag_with_latest || container_with_latest == tag_with_latest
                })
            });
        }
        
        Ok(images)
    }

    async fn remove_image(&self, image_id: &str) -> Result<()> {
        // Primeiro verifica se a imagem está em uso
        let images = self.list_images().await?;
        let image_in_use = images.iter().find(|img| img.id.starts_with(image_id) || 
            img.tags.iter().any(|tag| tag.contains(image_id)))
            .map(|img| img.in_use)
            .unwrap_or(false);
            
        if image_in_use {
            return Err(anyhow::anyhow!("Não é possível remover esta imagem pois ela está sendo usada por um ou mais containers"));
        }
        
        self.execute_docker_command(&format!("rmi {}", image_id))
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
                        in_use: false, // Será calculado posteriormente
                    });
                }
                Err(_) => {}
            }
        }

        // Verifica quais networks estão em uso por containers
        for network in &mut networks {
            network.in_use = self.is_network_used_by_containers(&network.name).await;
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
                        in_use: false, // Será calculado posteriormente
                    });
                }
                Err(_) => {}
            }
        }

        // Verifica quais volumes estão em uso por containers
        for volume in &mut volumes {
            volume.in_use = self.is_volume_used_by_containers(&volume.name).await;
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

impl DockerManager {
    // Função auxiliar para verificar se uma network é usada por containers
    async fn is_network_used_by_containers(&self, network_name: &str) -> bool {
        // Usa docker network inspect para verificar se há containers conectados
        match self.execute_docker_command(&format!("network inspect {}", network_name)).await {
            Ok(output) => {
                // Parse do JSON retornado pelo inspect
                if let Ok(networks) = serde_json::from_str::<Vec<serde_json::Value>>(&output) {
                    if let Some(network) = networks.first() {
                        if let Some(containers) = network.get("Containers") {
                            // Se há containers conectados, o objeto não estará vazio
                            return !containers.as_object().map_or(true, |obj| obj.is_empty());
                        }
                    }
                }
                false
            }
            Err(_) => false, // Se não conseguir inspecionar, assume que não está em uso
        }
    }

    // Função auxiliar para verificar se um volume é usado por containers
    async fn is_volume_used_by_containers(&self, volume_name: &str) -> bool {
        // Usa docker volume inspect para verificar se há containers conectados
        match self.execute_docker_command(&format!("volume inspect {}", volume_name)).await {
            Ok(output) => {
                // Parse do JSON retornado pelo inspect
                if let Ok(volumes) = serde_json::from_str::<Vec<serde_json::Value>>(&output) {
                    if let Some(volume) = volumes.first() {
                        // Para volumes, verificamos o RefCount (número de referências)
                        if let Some(ref_count) = volume.get("RefCount") {
                            return ref_count.as_i64().unwrap_or(0) > 0;
                        }
                        
                        // Alternativamente, verifica o campo UsageData
                        if let Some(usage_data) = volume.get("UsageData") {
                            if let Some(ref_count) = usage_data.get("RefCount") {
                                return ref_count.as_i64().unwrap_or(0) > 0;
                            }
                        }
                    }
                }
                false
            }
            Err(_) => false, // Se não conseguir inspecionar, assume que não está em uso
        }
    }
}
