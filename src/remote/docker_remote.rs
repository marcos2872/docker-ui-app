use crate::ssh::{SshDockerClient, SshServerConfig};
use crate::docker::{ContainerInfo, ImageInfo, NetworkInfo, VolumeInfo, DockerManager};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Informações de sistema Docker remoto
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteDockerSystemInfo {
    pub version: String,
    pub api_version: String,
    pub os: String,
    pub architecture: String,
    pub kernel_version: String,
    pub total_memory: u64,
    pub ncpu: u32,
    pub containers_running: usize,
    pub containers_paused: usize,
    pub containers_stopped: usize,
    pub images: usize,
    pub server_version: String,
}

/// Wrapper para gerenciar Docker via SSH (Adapter Pattern)
/// Adapta comandos Docker locais para execução remota via SSH
#[derive(Debug)]
pub struct DockerRemoteAdapter {
    ssh_client: Arc<SshDockerClient>,
    ssh_config: SshServerConfig,
    connected: Arc<RwLock<bool>>,
    system_info: Arc<RwLock<Option<RemoteDockerSystemInfo>>>,
}

impl DockerRemoteAdapter {
    /// Cria novo adapter para Docker remoto
    pub fn new(ssh_config: SshServerConfig) -> Self {
        let ssh_client = Arc::new(SshDockerClient::new(ssh_config.clone()));
        
        Self {
            ssh_client,
            ssh_config,
            connected: Arc::new(RwLock::new(false)),
            system_info: Arc::new(RwLock::new(None)),
        }
    }

    /// Conecta ao Docker remoto via SSH
    pub async fn connect(&self) -> Result<()> {
        // Placeholder - implementação SSH real viria aqui
        // Em uma implementação real, criaria conexão SSH e testaria comando docker
        
        let mut connected = self.connected.write().await;
        *connected = true;

        // Simula obtenção de informações do sistema
        let system_info = RemoteDockerSystemInfo {
            version: "24.0.0".to_string(),
            api_version: "1.43".to_string(),
            os: "linux".to_string(),
            architecture: "x86_64".to_string(),
            kernel_version: "5.15.0".to_string(),
            total_memory: 8589934592, // 8GB
            ncpu: 4,
            containers_running: 0,
            containers_paused: 0,
            containers_stopped: 0,
            images: 0,
            server_version: "Docker Engine - Community".to_string(),
        };

        let mut info = self.system_info.write().await;
        *info = Some(system_info);

        Ok(())
    }

    /// Desconecta do Docker remoto
    pub async fn disconnect(&self) -> Result<()> {
        let mut connected = self.connected.write().await;
        *connected = false;

        let mut info = self.system_info.write().await;
        *info = None;

        Ok(())
    }

    /// Verifica se está conectado
    pub async fn is_connected(&self) -> bool {
        let connected = self.connected.read().await;
        *connected
    }

    /// Testa conectividade com Docker remoto
    pub async fn test_connection(&self) -> Result<bool> {
        // Placeholder - implementação real executaria 'docker version' via SSH
        // ssh.execute_command("docker version --format json").await
        Ok(false) // Por ora retorna false
    }

    /// Obtém informações do sistema Docker remoto
    pub async fn get_system_info(&self) -> Option<RemoteDockerSystemInfo> {
        let info = self.system_info.read().await;
        info.clone()
    }

    /// Executa comando Docker remoto genérico
    async fn execute_docker_command(&self, _command: &str) -> Result<String> {
        let connected = self.connected.read().await;
        if !*connected {
            return Err(anyhow::anyhow!("Não conectado ao servidor remoto"));
        }

        // Placeholder - implementação SSH real viria aqui
        // let full_command = format!("docker {}", command);
        // self.ssh_client.execute_command(&full_command).await
        
        Ok("{}".to_string()) // Retorna JSON vazio como placeholder
    }

    /// Lista containers remotos
    pub async fn list_containers(&self) -> Result<Vec<ContainerInfo>> {
        let _output = self.execute_docker_command("ps --all --format json").await?;
        
        // Placeholder - parsing real do JSON viria aqui
        // let containers: Vec<ContainerInfo> = serde_json::from_str(&output)?;
        
        Ok(Vec::new()) // Retorna lista vazia como placeholder
    }

    /// Lista imagens remotas
    pub async fn list_images(&self) -> Result<Vec<ImageInfo>> {
        let _output = self.execute_docker_command("images --format json").await?;
        
        // Placeholder - parsing real do JSON viria aqui
        Ok(Vec::new())
    }

    /// Lista redes remotas
    pub async fn list_networks(&self) -> Result<Vec<NetworkInfo>> {
        let _output = self.execute_docker_command("network ls --format json").await?;
        
        // Placeholder - parsing real do JSON viria aqui
        Ok(Vec::new())
    }

    /// Lista volumes remotos
    pub async fn list_volumes(&self) -> Result<Vec<VolumeInfo>> {
        let _output = self.execute_docker_command("volume ls --format json").await?;
        
        // Placeholder - parsing real do JSON viria aqui
        Ok(Vec::new())
    }

    /// Inicia container remoto
    pub async fn start_container(&self, container_name: &str) -> Result<()> {
        let _output = self.execute_docker_command(&format!("start {}", container_name)).await?;
        Ok(())
    }

    /// Para container remoto
    pub async fn stop_container(&self, container_name: &str) -> Result<()> {
        let _output = self.execute_docker_command(&format!("stop {}", container_name)).await?;
        Ok(())
    }

    /// Reinicia container remoto
    pub async fn restart_container(&self, container_name: &str) -> Result<()> {
        let _output = self.execute_docker_command(&format!("restart {}", container_name)).await?;
        Ok(())
    }

    /// Remove container remoto
    pub async fn remove_container(&self, container_name: &str, force: bool) -> Result<()> {
        let force_flag = if force { " --force" } else { "" };
        let _output = self.execute_docker_command(&format!("rm{} {}", force_flag, container_name)).await?;
        Ok(())
    }

    /// Remove imagem remota
    pub async fn remove_image(&self, image_name: &str, force: bool) -> Result<()> {
        let force_flag = if force { " --force" } else { "" };
        let _output = self.execute_docker_command(&format!("rmi{} {}", force_flag, image_name)).await?;
        Ok(())
    }

    /// Remove rede remota
    pub async fn remove_network(&self, network_name: &str) -> Result<()> {
        let _output = self.execute_docker_command(&format!("network rm {}", network_name)).await?;
        Ok(())
    }

    /// Remove volume remoto
    pub async fn remove_volume(&self, volume_name: &str) -> Result<()> {
        let _output = self.execute_docker_command(&format!("volume rm {}", volume_name)).await?;
        Ok(())
    }

    /// Puxa imagem remota
    pub async fn pull_image(&self, image_name: &str) -> Result<()> {
        let _output = self.execute_docker_command(&format!("pull {}", image_name)).await?;
        Ok(())
    }

    /// Obtém logs de container remoto
    pub async fn get_container_logs(&self, container_name: &str, lines: Option<usize>) -> Result<String> {
        let lines_param = lines.map(|n| format!(" --tail {}", n)).unwrap_or_default();
        let output = self.execute_docker_command(&format!("logs{} {}", lines_param, container_name)).await?;
        Ok(output)
    }

    /// Inspeciona container remoto
    pub async fn inspect_container(&self, container_name: &str) -> Result<serde_json::Value> {
        let _output = self.execute_docker_command(&format!("inspect {}", container_name)).await?;
        
        // Placeholder - parsing real do JSON viria aqui
        Ok(serde_json::Value::Null)
    }

    /// Obtém estatísticas de container remoto
    pub async fn get_container_stats(&self, container_name: &str) -> Result<serde_json::Value> {
        let _output = self.execute_docker_command(&format!("stats {} --no-stream --format json", container_name)).await?;
        
        // Placeholder - parsing real do JSON viria aqui
        Ok(serde_json::Value::Null)
    }

    /// Cria container remoto
    pub async fn create_container(&self, image: &str, name: Option<&str>, options: &[String]) -> Result<String> {
        let name_param = name.map(|n| format!(" --name {}", n)).unwrap_or_default();
        let options_str = options.join(" ");
        let command = format!("create{} {} {}", name_param, options_str, image);
        
        let output = self.execute_docker_command(&command).await?;
        Ok(output.trim().to_string()) // Retorna ID do container
    }

    /// Executa comando em container remoto
    pub async fn exec_container(&self, container_name: &str, command: &[String]) -> Result<String> {
        let cmd_str = command.join(" ");
        let output = self.execute_docker_command(&format!("exec {} {}", container_name, cmd_str)).await?;
        Ok(output)
    }

    /// Cria rede remota
    pub async fn create_network(&self, name: &str, driver: Option<&str>) -> Result<()> {
        let driver_param = driver.map(|d| format!(" --driver {}", d)).unwrap_or_default();
        let _output = self.execute_docker_command(&format!("network create{} {}", driver_param, name)).await?;
        Ok(())
    }

    /// Cria volume remoto
    pub async fn create_volume(&self, name: &str, driver: Option<&str>) -> Result<()> {
        let driver_param = driver.map(|d| format!(" --driver {}", d)).unwrap_or_default();
        let _output = self.execute_docker_command(&format!("volume create{} {}", driver_param, name)).await?;
        Ok(())
    }

    /// Obtém uso de espaço Docker remoto
    pub async fn get_system_df(&self) -> Result<serde_json::Value> {
        let _output = self.execute_docker_command("system df --format json").await?;
        
        // Placeholder - parsing real do JSON viria aqui
        Ok(serde_json::Value::Null)
    }

    /// Limpa sistema Docker remoto
    pub async fn system_prune(&self, volumes: bool, images: bool) -> Result<String> {
        let mut flags = Vec::new();
        if volumes {
            flags.push("--volumes");
        }
        if images {
            flags.push("--all");
        }
        flags.push("--force"); // Para não pedir confirmação
        
        let flags_str = flags.join(" ");
        let output = self.execute_docker_command(&format!("system prune {}", flags_str)).await?;
        Ok(output)
    }

    /// Obtém configuração SSH usada
    pub fn get_ssh_config(&self) -> &SshServerConfig {
        &self.ssh_config
    }

    /// Obtém nome do servidor
    pub fn get_server_name(&self) -> &str {
        &self.ssh_config.name
    }

    /// Obtém endereço do servidor
    pub fn get_server_address(&self) -> String {
        format!("{}:{}", self.ssh_config.host, self.ssh_config.port)
    }
}

/// Factory para criar adapters Docker remotos
pub struct DockerRemoteFactory;

impl DockerRemoteFactory {
    /// Cria adapter para servidor remoto
    pub fn create_remote_adapter(ssh_config: SshServerConfig) -> DockerRemoteAdapter {
        DockerRemoteAdapter::new(ssh_config)
    }

    /// Cria adapter para servidor local (wrapper sobre DockerManager local)
    pub async fn create_local_adapter() -> Result<Arc<DockerManager>> {
        // Para servidor local, retorna o DockerManager padrão
        let manager = DockerManager::new().await?;
        Ok(Arc::new(manager))
    }
}

/// Trait para uniformizar interface entre Docker local e remoto
#[async_trait::async_trait]
pub trait DockerInterface {
    async fn list_containers(&self) -> Result<Vec<ContainerInfo>>;
    async fn list_images(&self) -> Result<Vec<ImageInfo>>;
    async fn list_networks(&self) -> Result<Vec<NetworkInfo>>;
    async fn list_volumes(&self) -> Result<Vec<VolumeInfo>>;
    
    async fn start_container(&self, container_name: &str) -> Result<()>;
    async fn stop_container(&self, container_name: &str) -> Result<()>;
    async fn restart_container(&self, container_name: &str) -> Result<()>;
    async fn remove_container(&self, container_name: &str, force: bool) -> Result<()>;
    
    async fn remove_image(&self, image_name: &str, force: bool) -> Result<()>;
    async fn remove_network(&self, network_name: &str) -> Result<()>;
    async fn remove_volume(&self, volume_name: &str) -> Result<()>;
}

/// Implementação do trait para adapter remoto
#[async_trait::async_trait]
impl DockerInterface for DockerRemoteAdapter {
    async fn list_containers(&self) -> Result<Vec<ContainerInfo>> {
        self.list_containers().await
    }

    async fn list_images(&self) -> Result<Vec<ImageInfo>> {
        self.list_images().await
    }

    async fn list_networks(&self) -> Result<Vec<NetworkInfo>> {
        self.list_networks().await
    }

    async fn list_volumes(&self) -> Result<Vec<VolumeInfo>> {
        self.list_volumes().await
    }

    async fn start_container(&self, container_name: &str) -> Result<()> {
        self.start_container(container_name).await
    }

    async fn stop_container(&self, container_name: &str) -> Result<()> {
        self.stop_container(container_name).await
    }

    async fn restart_container(&self, container_name: &str) -> Result<()> {
        self.restart_container(container_name).await
    }

    async fn remove_container(&self, container_name: &str, force: bool) -> Result<()> {
        self.remove_container(container_name, force).await
    }

    async fn remove_image(&self, image_name: &str, force: bool) -> Result<()> {
        self.remove_image(image_name, force).await
    }

    async fn remove_network(&self, network_name: &str) -> Result<()> {
        self.remove_network(network_name).await
    }

    async fn remove_volume(&self, volume_name: &str) -> Result<()> {
        self.remove_volume(volume_name).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ssh::AuthMethod;
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_create_remote_adapter() {
        let ssh_config = SshServerConfig::new_with_password(
            "test_server".to_string(),
            "localhost".to_string(),
            22,
            "user".to_string(),
            "password".to_string(),
        );

        let adapter = DockerRemoteAdapter::new(ssh_config);
        assert!(!adapter.is_connected().await);
        assert_eq!(adapter.get_server_name(), "test_server");
    }

    #[tokio::test]
    async fn test_connection_lifecycle() {
        let ssh_config = SshServerConfig::new_with_password(
            "test_server".to_string(),
            "localhost".to_string(),
            22,
            "user".to_string(),
            "password".to_string(),
        );

        let adapter = DockerRemoteAdapter::new(ssh_config);
        
        // Inicialmente desconectado
        assert!(!adapter.is_connected().await);
        
        // Conecta
        adapter.connect().await.unwrap();
        assert!(adapter.is_connected().await);
        
        // Obtém informações do sistema
        let info = adapter.get_system_info().await;
        assert!(info.is_some());
        
        // Desconecta
        adapter.disconnect().await.unwrap();
        assert!(!adapter.is_connected().await);
    }

    #[tokio::test]
    async fn test_factory_methods() {
        let ssh_config = SshServerConfig::new_with_password(
            "test_server".to_string(),
            "localhost".to_string(),
            22,
            "user".to_string(),
            "password".to_string(),
        );

        let remote_adapter = DockerRemoteFactory::create_remote_adapter(ssh_config);
        assert_eq!(remote_adapter.get_server_name(), "test_server");

        // Note: create_local_adapter pode falhar se Docker não estiver disponível
        // let local_adapter = DockerRemoteFactory::create_local_adapter().await;
        // assert!(local_adapter.is_ok() || local_adapter.is_err()); // Qualquer resultado é válido para teste
    }
}