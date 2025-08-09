use crate::docker::{DockerManager, NetworkInfo};
use std::sync::Arc;

// Struct para dados das networks no formato Slint
#[derive(Clone, Debug, Default)]
pub struct SlintNetworkData {
    pub id: slint::SharedString,
    pub name: slint::SharedString,
    pub driver: slint::SharedString,
    pub scope: slint::SharedString,
    pub created: slint::SharedString,
    pub containers_count: i32,
    pub is_system: bool,
}

impl From<&NetworkInfo> for SlintNetworkData {
    fn from(network: &NetworkInfo) -> Self {
        Self {
            id: network.id.clone().into(),
            name: network.name.clone().into(),
            driver: network.driver.clone().into(),
            scope: network.scope.clone().into(),
            created: format_creation_time(&network.created),
            containers_count: network.containers_count,
            is_system: network.is_system,
        }
    }
}

// Gerenciador da UI de networks
pub struct NetworkUIManager {
    docker_manager: Arc<tokio::sync::Mutex<DockerManager>>,
    networks: Vec<NetworkInfo>,
}

impl NetworkUIManager {
    pub fn new(docker_manager: Arc<tokio::sync::Mutex<DockerManager>>) -> Self {
        Self {
            docker_manager,
            networks: Vec::new(),
        }
    }

    // Atualiza a lista de networks
    pub async fn refresh_networks(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let docker_manager = self.docker_manager.lock().await;
        match docker_manager.list_networks().await {
            Ok(networks) => {
                self.networks = networks;
                Ok(())
            }
            Err(e) => Err(format!("Failed to refresh networks: {}", e).into()),
        }
    }

    // Retorna a lista de networks para a UI
    pub fn get_networks(&self) -> Vec<SlintNetworkData> {
        self.networks.iter().map(SlintNetworkData::from).collect()
    }

    // Executa ação em uma network
    pub async fn execute_network_action(
        &self,
        network_id: &str,
        action: &str,
    ) -> Result<String, String> {
        let docker_manager = self.docker_manager.lock().await;

        match action {
            "remove" => match docker_manager.remove_network(network_id).await {
                Ok(_) => Ok("Network removida com sucesso.".to_string()),
                Err(e) => Err(e.to_string()),
            },
            _ => Err(format!("Unknown action: {}", action)),
        }
    }
}

// Formata o tempo de criação
fn format_creation_time(created: &str) -> slint::SharedString {
    if created.is_empty() {
        return "desconhecido".into();
    }

    use chrono::{DateTime, Utc};
    
    // Tenta fazer parse do timestamp ISO8601 do Docker
    match DateTime::parse_from_rfc3339(created) {
        Ok(created_time) => {
            let now = Utc::now();
            let created_utc = created_time.with_timezone(&Utc);
            let duration = now.signed_duration_since(created_utc);
            
            let days = duration.num_days();
            let hours = duration.num_hours();
            let minutes = duration.num_minutes();
            let seconds = duration.num_seconds();
            
            if days > 0 {
                format!("há {} dia{}", days, if days == 1 { "" } else { "s" }).into()
            } else if hours > 0 {
                format!("há {} hora{}", hours, if hours == 1 { "" } else { "s" }).into()
            } else if minutes > 0 {
                format!("há {} minuto{}", minutes, if minutes == 1 { "" } else { "s" }).into()
            } else if seconds > 0 {
                format!("há {} segundo{}", seconds, if seconds == 1 { "" } else { "s" }).into()
            } else {
                "agora".into()
            }
        }
        Err(_) => "desconhecido".into(),
    }
}