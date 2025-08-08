use crate::docker::{DockerManager, ImageInfo};
use std::sync::Arc;

// Struct para dados das imagens no formato Slint
#[derive(Clone, Debug, Default)]
pub struct SlintImageData {
    pub id: slint::SharedString,
    pub tag: slint::SharedString,
    pub size: slint::SharedString,
    pub created: slint::SharedString,
    pub in_use: bool,
}

impl From<&ImageInfo> for SlintImageData {
    fn from(image: &ImageInfo) -> Self {
        Self {
            id: image.id.clone().into(),
            tag: image.tags.get(0).cloned().unwrap_or_default().into(),
            size: format_size(image.size),
            created: format_creation_time(image.created),
            in_use: image.in_use,
        }
    }
}

// Gerenciador da UI de imagens
pub struct ImageUIManager {
    docker_manager: Arc<tokio::sync::Mutex<DockerManager>>,
    images: Vec<ImageInfo>,
}

impl ImageUIManager {
    pub fn new(docker_manager: Arc<tokio::sync::Mutex<DockerManager>>) -> Self {
        Self {
            docker_manager,
            images: Vec::new(),
        }
    }

    // Atualiza a lista de imagens
    pub async fn refresh_images(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let docker_manager = self.docker_manager.lock().await;
        match docker_manager.list_images().await {
            Ok(images) => {
                self.images = images;
                Ok(())
            }
            Err(e) => Err(format!("Failed to refresh images: {}", e).into()),
        }
    }

    // Retorna a lista de imagens para a UI
    pub fn get_images(&self) -> Vec<SlintImageData> {
        self.images.iter().map(SlintImageData::from).collect()
    }

    // Executa ação em uma imagem
    pub async fn execute_image_action(
        &self,
        image_id: &str,
        action: &str,
    ) -> Result<String, String> {
        let docker_manager = self.docker_manager.lock().await;

        match action {
            "remove" => match docker_manager.remove_image(image_id).await {
                Ok(_) => Ok("Imagem removida com sucesso.".to_string()),
                Err(e) => Err(e.to_string()),
            },
            _ => Err(format!("Unknown action: {}", action)),
        }
    }
}

// Formata o tamanho do arquivo
fn format_size(size: i64) -> slint::SharedString {
    if size <= 0 {
        return "0 B".into();
    }
    let units = ["B", "KB", "MB", "GB", "TB"];
    let digit_groups = (size as f64).log10() / (1024_f64).log10();
    let unit_index = digit_groups.floor() as i32;
    let size_in_unit = size as f64 / 1024_f64.powi(unit_index);
    format!("{:.2} {}", size_in_unit, units[unit_index as usize]).into()
}

// Formata o tempo de criação
fn format_creation_time(created: i64) -> slint::SharedString {
    if created > 0 {
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
