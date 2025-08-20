use crate::docker::{DockerManager, VolumeInfo};
use std::sync::Arc;

// Struct para dados dos volumes no formato Slint
#[derive(Clone, Debug, Default)]
pub struct SlintVolumeData {
    pub name: slint::SharedString,
    pub driver: slint::SharedString,
    pub mountpoint: slint::SharedString,
    pub created: slint::SharedString,
    pub containers_count: i32,
}

impl From<&VolumeInfo> for SlintVolumeData {
    fn from(volume: &VolumeInfo) -> Self {
        Self {
            name: volume.name.clone().into(),
            driver: volume.driver.clone().into(),
            mountpoint: volume.mountpoint.clone().into(),
            created: format_creation_time(&volume.created),
            containers_count: volume.containers_count,
        }
    }
}

// Gerenciador da UI de volumes
pub struct VolumeUIManager {
    docker_manager: Arc<tokio::sync::Mutex<DockerManager>>,
    volumes: Vec<VolumeInfo>,
}

impl VolumeUIManager {
    pub fn new(docker_manager: Arc<tokio::sync::Mutex<DockerManager>>) -> Self {
        Self {
            docker_manager,
            volumes: Vec::new(),
        }
    }

    // Atualiza a lista de volumes
    pub async fn refresh_volumes(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let docker_manager = self.docker_manager.lock().await;
        match docker_manager.list_volumes().await {
            Ok(volumes) => {
                self.volumes = volumes;
                Ok(())
            }
            Err(e) => Err(format!("Failed to refresh volumes: {}", e).into()),
        }
    }

    // Retorna a lista de volumes para a UI
    pub fn get_volumes(&self) -> Vec<SlintVolumeData> {
        self.volumes.iter().map(SlintVolumeData::from).collect()
    }

    // Executa ação em um volume
    pub async fn execute_volume_action(
        &self,
        volume_name: &str,
        action: &str,
    ) -> Result<String, String> {
        let docker_manager = self.docker_manager.lock().await;

        match action {
            "remove" => match docker_manager.remove_volume(volume_name).await {
                Ok(_) => Ok("Volume removido com sucesso.".to_string()),
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
                format!(
                    "há {} minuto{}",
                    minutes,
                    if minutes == 1 { "" } else { "s" }
                )
                .into()
            } else if seconds > 0 {
                format!(
                    "há {} segundo{}",
                    seconds,
                    if seconds == 1 { "" } else { "s" }
                )
                .into()
            } else {
                "agora".into()
            }
        }
        Err(_) => "desconhecido".into(),
    }
}
