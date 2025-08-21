use crate::docker::{ContainerInfo, DockerManager, DockerManagement};
use slint::{Timer, TimerMode};
use std::sync::Arc;
use std::time::Duration;

// Struct para dados dos containers no formato Slint
#[derive(Clone, Debug)]
pub struct SlintContainerData {
    pub name: slint::SharedString,
    pub image: slint::SharedString,
    pub status: slint::SharedString,
    pub ports: slint::SharedString,
    pub running_for: slint::SharedString,
}

impl From<&ContainerInfo> for SlintContainerData {
    fn from(container: &ContainerInfo) -> Self {
        let ports_str = if container.ports.is_empty() {
            "Nenhuma".to_string()
        } else {
            extract_ports_only(&container.ports)
        };

        let running_for_str = if container.running_for.is_empty() {
            "-".to_string()
        } else {
            container.running_for.clone()
        };

        Self {
            name: container.name.clone().into(),
            image: container.image.clone().into(),
            status: parse_container_status(&container.state, &container.status),
            ports: ports_str.into(),
            running_for: running_for_str.into(),
        }
    }
}

// Gerenciador da UI de containers
pub struct ContainerUIManager {
    docker_manager: Arc<tokio::sync::Mutex<DockerManager>>,
    containers: Vec<ContainerInfo>,
    search_filter: String,
    status_filter: String,
}

impl ContainerUIManager {
    pub fn new(docker_manager: Arc<tokio::sync::Mutex<DockerManager>>) -> Self {
        Self {
            docker_manager,
            containers: Vec::new(),
            search_filter: String::new(),
            status_filter: "all".to_string(),
        }
    }

    // Atualiza a lista de containers
    pub async fn refresh_containers(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let docker_manager = self.docker_manager.lock().await;
        match docker_manager.list_containers().await {
            Ok(containers) => {
                self.containers = containers;
                Ok(())
            }
            Err(e) => Err(format!("Failed to refresh containers: {}", e).into()),
        }
    }

    // Filtra containers baseado nos critérios atuais
    pub fn get_filtered_containers(&self) -> Vec<SlintContainerData> {
        self.containers
            .iter()
            .filter(|container| {
                // Apply search filter
                let matches_search = if self.search_filter.trim().is_empty() {
                    true
                } else {
                    container
                        .name
                        .to_lowercase()
                        .contains(&self.search_filter.to_lowercase())
                        || container
                            .image
                            .to_lowercase()
                            .contains(&self.search_filter.to_lowercase())
                };

                // Apply status filter
                let container_status = parse_container_status(&container.state, &container.status);
                let matches_status = match self.status_filter.as_str() {
                    "all" => true,
                    "running" => container_status == "running",
                    "exited" => container_status == "exited",
                    "paused" => container_status == "paused",
                    _ => true,
                };

                matches_search && matches_status
            })
            .map(SlintContainerData::from)
            .collect()
    }

    // Atualiza filtro de busca
    pub fn set_search_filter(&mut self, search: String) {
        self.search_filter = search;
    }

    // Atualiza filtro de status
    pub fn set_status_filter(&mut self, status: String) {
        self.status_filter = status;
    }

    // Executa ação em um container
    pub async fn execute_container_action(
        &self,
        container_name: &str,
        action: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let docker_manager = self.docker_manager.lock().await;

        match action {
            "start" => docker_manager
                .start_container(container_name)
                .await
                .map_err(|e| format!("Failed to start container: {}", e).into()),
            "stop" => docker_manager
                .stop_container(container_name)
                .await
                .map_err(|e| format!("Failed to stop container: {}", e).into()),
            "pause" => docker_manager
                .pause_container(container_name)
                .await
                .map_err(|e| format!("Failed to pause container: {}", e).into()),
            "unpause" => docker_manager
                .unpause_container(container_name)
                .await
                .map_err(|e| format!("Failed to unpause container: {}", e).into()),
            "remove" => docker_manager
                .remove_container(container_name)
                .await
                .map_err(|e| format!("Failed to remove container: {}", e).into()),
            "restart" => docker_manager
                .restart_container(container_name)
                .await
                .map_err(|e| format!("Failed to restart container: {}", e).into()),
            _ => Err(format!("Unknown action: {}", action).into()),
        }
    }
}

// Converte o status do container para formato mais amigável
fn parse_container_status(state: &str, status: &str) -> slint::SharedString {
    let state_lower = state.to_lowercase();
    let status_lower = status.to_lowercase();

    if state_lower == "running" {
        "running".into()
    } else if state_lower == "exited" || status_lower.contains("exited") {
        "exited".into()
    } else if state_lower == "paused" || status_lower.contains("paused") {
        "paused".into()
    } else if state_lower == "restarting" {
        "restarting".into()
    } else {
        "unknown".into()
    }
}

// Extrai mapeamento de portas no formato host:container sem IP e sem duplicatas
// Exemplo: "0.0.0.0:8080->80/tcp, 443->443/tcp" -> "8080:80, 443:443" 
fn extract_ports_only(ports_str: &str) -> String {
    if ports_str.is_empty() {
        return "Nenhuma".to_string();
    }

    let mut unique_ports = std::collections::HashSet::new();
    
    for port_mapping in ports_str.split(',') {
        let trimmed = port_mapping.trim();
        
        // Remove protocolo primeiro se existir: "xxx/tcp" -> "xxx"
        let without_protocol = if let Some(slash_pos) = trimmed.find('/') {
            &trimmed[..slash_pos]
        } else {
            trimmed
        };
        
        // Processa diferentes formatos de mapeamento de porta
        if let Some(arrow_pos) = without_protocol.find("->") {
            // Formato "0.0.0.0:8080->80" ou "8080->80"
            let host_part = &without_protocol[..arrow_pos];
            let container_part = &without_protocol[arrow_pos + 2..];
            
            // Remove IP se presente: "0.0.0.0:8080" -> "8080"
            let host_port = if let Some(colon_pos) = host_part.rfind(':') {
                &host_part[colon_pos + 1..]
            } else {
                host_part
            };
            
            unique_ports.insert(format!("{}:{}", host_port, container_part));
        } else if without_protocol.contains(':') && !without_protocol.contains('.') {
            // Formato simples "8080:80" (sem IP)
            unique_ports.insert(without_protocol.to_string());
        } else if !without_protocol.is_empty() && without_protocol.chars().all(|c| c.is_ascii_digit()) {
            // Apenas número, assume porta exposta: "80" -> "80:80"
            unique_ports.insert(format!("{}:{}", without_protocol, without_protocol));
        }
    }

    if unique_ports.is_empty() {
        "Nenhuma".to_string()
    } else {
        let mut ports: Vec<String> = unique_ports.into_iter().collect();
        ports.sort(); // Ordena para ter resultado consistente
        ports.join(", ")
    }
}

// Configura timer para atualização automática da UI
pub fn setup_container_ui_timer(
    ui_manager: Arc<tokio::sync::Mutex<ContainerUIManager>>,
    update_callback: Arc<dyn Fn(Vec<SlintContainerData>) + Send + Sync>,
) -> Timer {
    let timer = Timer::default();

    timer.start(TimerMode::Repeated, Duration::from_secs(2), move || {
        let ui_manager_clone = ui_manager.clone();
        let callback_clone = update_callback.clone();

        tokio::spawn(async move {
            let mut manager = ui_manager_clone.lock().await;
            if let Ok(()) = manager.refresh_containers().await {
                let filtered_containers = manager.get_filtered_containers();

                slint::invoke_from_event_loop(move || {
                    callback_clone(filtered_containers);
                })
                .unwrap();
            }
        });
    });

    timer
}

// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[test]
//     fn test_parse_container_status() {
//         assert_eq!(parse_container_status("running", "Up 5 minutes"), "running");
//         assert_eq!(
//             parse_container_status("exited", "Exited (0) 2 minutes ago"),
//             "exited"
//         );
//         assert_eq!(
//             parse_container_status("paused", "Up 1 hour (Paused)"),
//             "paused"
//         );
//         assert_eq!(
//             parse_container_status("restarting", "Restarting"),
//             "restarting"
//         );
//         assert_eq!(parse_container_status("created", "Created"), "unknown");
//     }
// }
