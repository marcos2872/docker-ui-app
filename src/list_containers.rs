use crate::docker::{ContainerInfo, DockerManager};
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
    pub created: slint::SharedString,
}

impl From<&ContainerInfo> for SlintContainerData {
    fn from(container: &ContainerInfo) -> Self {
        let ports_str = if container.ports.is_empty() {
            "Nenhuma".to_string()
        } else {
            container
                .ports
                .iter()
                .map(|p| p.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        };

        Self {
            name: container.name.clone().into(),
            image: container.image.clone().into(),
            status: parse_container_status(&container.state, &container.status),
            ports: ports_str.into(),
            created: format_creation_time(container.created),
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

// Formata o tempo de criação
fn format_creation_time(created: i64) -> slint::SharedString {
    if created > 0 {
        // Convert Unix timestamp to readable format
        use std::time::{Duration, SystemTime, UNIX_EPOCH};

        let timestamp = UNIX_EPOCH + Duration::from_secs(created as u64);
        let now = SystemTime::now();

        match now.duration_since(timestamp) {
            Ok(duration) => {
                let days = duration.as_secs() / (24 * 3600);
                let hours = (duration.as_secs() % (24 * 3600)) / 3600;
                let minutes = (duration.as_secs() % 3600) / 60;

                if days > 0 {
                    format!("há {} dias", days).into()
                } else if hours > 0 {
                    format!("há {} horas", hours).into()
                } else if minutes > 0 {
                    format!("há {} min", minutes).into()
                } else {
                    "agora".into()
                }
            }
            Err(_) => "desconhecido".into(),
        }
    } else {
        "desconhecido".into()
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
