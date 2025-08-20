use crate::docker::{ContainerInfo, DockerInfo, ImageInfo, NetworkInfo, VolumeInfo};
use crate::ssh::{SshClient, SshConnection};
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde_json;

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
    async fn list_volumes(&self) -> Result<Vec<VolumeInfo>>;
    async fn get_docker_info(&self) -> Result<DockerInfo>;
    async fn get_container_logs(
        &self,
        container_name: &str,
        lines: Option<usize>,
    ) -> Result<String>;
}

pub struct RemoteDockerManager {
    ssh_client: SshClient,
    connected: bool,
}

impl RemoteDockerManager {
    pub fn new() -> Self {
        Self {
            ssh_client: SshClient::new(),
            connected: false,
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
}
