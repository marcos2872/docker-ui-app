use crate::docker::{DockerManager, DockerManagement};
use crate::ssh::SshConnection;
use crate::ui::setup_docker_ui;
use crate::{chart::ChartRenderer, AppState, ContainerChartData};
use crate::ssh_persistence::{SavedSshServer, SshPersistence};
use chrono::{DateTime, Local};
use slint::ComponentHandle;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

// Importa os tipos gerados pelo Slint
use crate::{AppWindow, SshServerData};

#[derive(Clone)]
pub struct SshUiState {
    pub persistence: Arc<SshPersistence>,
    pub connections: Arc<Mutex<HashMap<String, Arc<tokio::sync::Mutex<DockerManager>>>>>,
    pub current_connection: Arc<Mutex<Option<String>>>,
}

impl SshUiState {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let persistence = Arc::new(SshPersistence::new()?);

        Ok(Self {
            persistence,
            connections: Arc::new(Mutex::new(HashMap::new())),
            current_connection: Arc::new(Mutex::new(None)),
        })
    }

    pub fn load_servers(&self) -> Result<Vec<SavedSshServer>, Box<dyn std::error::Error>> {
        Ok(self.persistence.load_config()?.servers)
    }

    pub async fn connect_to_server(
        &self,
        server_id: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let server = self
            .persistence
            .get_server(server_id)?
            .ok_or("Server not found")?;

        let mut manager = DockerManager::new();
        let connection = server.to_ssh_connection();

        manager.connect(connection).await?;

        // Atualizar último acesso
        self.persistence.mark_as_connected(server_id)?;

        // Lista containers do servidor remoto
        let _ = manager.list_containers().await;

        // Guardar conexão ativa
        let manager = Arc::new(tokio::sync::Mutex::new(manager));
        {
            let mut connections = self.connections.lock().unwrap();
            connections.insert(server_id.to_string(), manager);
        }

        {
            let mut current = self.current_connection.lock().unwrap();
            *current = Some(server_id.to_string());
        }

        Ok(())
    }

    pub fn disconnect_current_server(&self) {
        let current_id = {
            let mut current = self.current_connection.lock().unwrap();
            current.take()
        };

        if let Some(server_id) = current_id {
            let mut connections = self.connections.lock().unwrap();
            if let Some(manager) = connections.remove(&server_id) {
                manager.blocking_lock().disconnect();
            }
        }
    }

    pub fn is_connected(&self) -> bool {
        let current = self.current_connection.lock().unwrap();
        current.is_some()
    }

    pub fn get_current_server_id(&self) -> Option<String> {
        let current = self.current_connection.lock().unwrap();
        current.clone()
    }

    pub fn has_current_connection(&self) -> bool {
        let current = self.current_connection.lock().unwrap();
        if let Some(server_id) = current.as_ref() {
            let connections = self.connections.lock().unwrap();
            connections.contains_key(server_id)
        } else {
            false
        }
    }
}

pub fn setup_ssh_ui(
    ui: &AppWindow,
    ssh_state: Arc<SshUiState>,
    app_state: AppState,
    container_chart_data: Arc<Mutex<ContainerChartData>>,
    container_cpu_renderer: Arc<Mutex<ChartRenderer>>,
    container_memory_renderer: Arc<Mutex<ChartRenderer>>,
) {
    // Converter SavedSshServer para SshServerData da UI
    let convert_to_ui_data = |server: SavedSshServer| -> SshServerData {
        let last_connected = server
            .last_connected
            .map(|dt| {
                let local: DateTime<Local> = dt.into();
                local.format("%d/%m/%Y %H:%M").to_string()
            })
            .unwrap_or_else(|| "Nunca".to_string());

        SshServerData {
            id: server.id.into(),
            name: server.name.into(),
            host: server.host.into(),
            port: server.port as i32,
            username: server.username.into(),
            password: server.password.into(),
            description: server.description.unwrap_or_default().into(),
            last_connected: last_connected.into(),
            is_favorite: server.is_favorite,
            is_connected: false,  // Será atualizado dinamicamente
            is_connecting: false, // Será atualizado dinamicamente
        }
    };

    // Carregar servidores iniciais
    let ssh_state_clone = ssh_state.clone();
    if let Ok(servers) = ssh_state_clone.load_servers() {
        let ui_servers: Vec<SshServerData> = servers.into_iter().map(convert_to_ui_data).collect();

        ui.set_ssh_servers(ui_servers.as_slice().into());
    }

    // Callback para conectar servidor
    let ssh_state_clone = ssh_state.clone();
    let ui_weak = ui.as_weak();
    ui.on_ssh_connect_server(move |server_id| {
        let ssh_state = ssh_state_clone.clone();
        let ui_weak = ui_weak.clone();
        let server_id = server_id.to_string();

        // Verificar se já está conectado a este servidor
        let current_connected = ssh_state.get_current_server_id().unwrap_or_default();
        let is_disconnect = current_connected == server_id;

        if is_disconnect {

            // Ativar loading para desconectar
            if let Some(ui) = ui_weak.upgrade() {
                if let Ok(servers) = ssh_state.load_servers() {
                    let current_id = ssh_state.get_current_server_id().unwrap_or_default();
                    let ui_servers: Vec<SshServerData> = servers
                        .into_iter()
                        .map(|server| {
                            let mut ui_data = convert_to_ui_data(server);
                            ui_data.is_connected = ui_data.id.as_str() == current_id;
                            ui_data.is_connecting = ui_data.id.as_str() == server_id; // Loading para o servidor sendo desconectado
                            ui_data
                        })
                        .collect();

                    ui.set_ssh_servers(ui_servers.as_slice().into());
                }
            }

            // Simular um pequeno delay para mostrar o loading (desconexão é instantânea)
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(500));

                slint::invoke_from_event_loop(move || {
                    // Desconectar
                    ssh_state.disconnect_current_server();

                    if let Some(ui) = ui_weak.upgrade() {
                        ui.set_current_ssh_server_id("".into());
                        ui.set_notification_message("Desconectado com sucesso!".into());
                        ui.set_notification_is_error(false);
                        ui.set_show_notification(true);

                        // Atualizar lista de servidores
                        if let Ok(servers) = ssh_state.load_servers() {
                            let ui_servers: Vec<SshServerData> = servers
                                .into_iter()
                                .map(|server| {
                                    let mut ui_data = convert_to_ui_data(server);
                                    ui_data.is_connected = false; // Nenhum conectado
                                    ui_data.is_connecting = false;
                                    ui_data
                                })
                                .collect();

                            ui.set_ssh_servers(ui_servers.as_slice().into());
                        }
                    }
                })
                .unwrap();
            });

            return;
        }


        // Ativar loading
        if let Some(ui) = ui_weak.upgrade() {
            if let Ok(servers) = ssh_state.load_servers() {
                let current_id = ssh_state.get_current_server_id().unwrap_or_default();
                let ui_servers: Vec<SshServerData> = servers
                    .into_iter()
                    .map(|server| {
                        let mut ui_data = convert_to_ui_data(server);
                        ui_data.is_connected = ui_data.id.as_str() == current_id;
                        ui_data.is_connecting = ui_data.id.as_str() == server_id;
                        ui_data
                    })
                    .collect();

                ui.set_ssh_servers(ui_servers.as_slice().into());
            }
        }

        let app_state_clone = app_state.clone();
        let container_chart_data_clone = container_chart_data.clone();
        let container_cpu_renderer_clone = container_cpu_renderer.clone();
        let container_memory_renderer_clone = container_memory_renderer.clone();

        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                match ssh_state.connect_to_server(&server_id).await {
                    Ok(_) => {
                        let server_id_success = server_id.clone();
                        let ssh_state_success = ssh_state.clone();

                        let docker_manager = ssh_state_success
                            .connections
                            .lock()
                            .unwrap()
                            .get(&server_id_success)
                            .unwrap()
                            .clone();

                        slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak.upgrade() {
                                ui.set_current_ssh_server_id(server_id_success.clone().into());
                                ui.set_notification_message("Conectado com sucesso!".into());
                                ui.set_notification_is_error(false);
                                ui.set_show_notification(true);

                                // Atualizar lista de servidores (desativa loading)
                                if let Ok(servers) = ssh_state_success.load_servers() {
                                    let current_id = ssh_state_success
                                        .get_current_server_id()
                                        .unwrap_or_default();
                                    let ui_servers: Vec<SshServerData> = servers
                                        .into_iter()
                                        .map(|server| {
                                            let mut ui_data = convert_to_ui_data(server);
                                            ui_data.is_connected =
                                                ui_data.id.as_str() == current_id;
                                            ui_data.is_connecting = false; // Desativa loading
                                            ui_data
                                        })
                                        .collect();

                                    ui.set_ssh_servers(ui_servers.as_slice().into());
                                }

                                // Setup Docker UI
                                setup_docker_ui(
                                    ui.as_weak(),
                                    app_state_clone,
                                    docker_manager,
                                    container_chart_data_clone,
                                    container_cpu_renderer_clone,
                                    container_memory_renderer_clone,
                                );
                            } else {
                            }
                        })
                        .unwrap();
                    }
                    Err(e) => {
                        let error_msg = std::format!("Erro na conexão: {}", e);

                        slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak.upgrade() {
                                ui.set_notification_message(error_msg.into());
                                ui.set_notification_is_error(true);
                                ui.set_show_notification(true);

                                // Desativar loading no caso de erro
                                if let Ok(servers) = ssh_state.load_servers() {
                                    let current_id = ssh_state.get_current_server_id().unwrap_or_default();
                                    let ui_servers: Vec<SshServerData> = servers
                                        .into_iter()
                                        .map(|server| {
                                            let mut ui_data = convert_to_ui_data(server);
                                            ui_data.is_connected = ui_data.id.as_str() == current_id;
                                            ui_data.is_connecting = false; // Desativa loading
                                            ui_data
                                        })
                                        .collect();

                                    ui.set_ssh_servers(ui_servers.as_slice().into());
                                }
                            } else {
                            }
                        }).unwrap();
                    }
                }
            });
        });
    });

    // Callback para salvar servidor
    let ssh_state_clone = ssh_state.clone();
    let ui_weak = ui.as_weak();
    ui.on_ssh_save_server(move |server_data| {
        let server = SavedSshServer {
            id: if server_data.id.is_empty() {
                uuid::Uuid::new_v4().to_string()
            } else {
                server_data.id.to_string()
            },
            name: server_data.name.to_string(),
            host: server_data.host.to_string(),
            port: server_data.port as u16,
            username: server_data.username.to_string(),
            password: server_data.password.to_string(),
            private_key_path: None,
            last_connected: None,
            is_favorite: server_data.is_favorite,
            description: if server_data.description.is_empty() {
                None
            } else {
                Some(server_data.description.to_string())
            },
        };

        match ssh_state_clone.persistence.add_server(server) {
            Ok(_) => {
                if let Some(ui) = ui_weak.upgrade() {
                    ui.set_ssh_is_saving(false);
                    ui.set_notification_message("Servidor salvo com sucesso!".into());
                    ui.set_notification_is_error(false);
                    ui.set_show_notification(true);

                    // Limpar formulário após salvar com sucesso
                    ui.set_ssh_server_data(SshServerData {
                        id: "".into(),
                        name: "".into(),
                        host: "".into(),
                        port: 22,
                        username: "".into(),
                        password: "".into(),
                        description: "".into(),
                        last_connected: "".into(),
                        is_favorite: false,
                        is_connected: false,
                        is_connecting: false,
                    });
                    ui.set_ssh_modal_visible(false);

                    // Recarregar lista
                    if let Ok(servers) = ssh_state_clone.load_servers() {
                        let current_id =
                            ssh_state_clone.get_current_server_id().unwrap_or_default();
                        let ui_servers: Vec<SshServerData> = servers
                            .into_iter()
                            .map(|server| {
                                let mut ui_data = convert_to_ui_data(server);
                                ui_data.is_connected = ui_data.id.as_str() == current_id;
                                ui_data
                            })
                            .collect();

                        ui.set_ssh_servers(ui_servers.as_slice().into());
                    }
                }
            }
            Err(e) => {
                if let Some(ui) = ui_weak.upgrade() {
                    ui.set_ssh_is_saving(false);
                    ui.set_notification_message(std::format!("Erro ao salvar: {}", e).into());
                    ui.set_notification_is_error(true);
                    ui.set_show_notification(true);
                }
            }
        }
    });

    // Callback para editar servidor
    let ssh_state_clone = ssh_state.clone();
    let ui_weak = ui.as_weak();
    ui.on_ssh_edit_server(move |server_id| {
        if let Ok(Some(server)) = ssh_state_clone
            .persistence
            .get_server(&server_id.to_string())
        {
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_ssh_server_data(convert_to_ui_data(server));
                ui.set_ssh_edit_mode(true);
                ui.set_ssh_modal_visible(true);
            }
        }
    });

    // Callback para deletar servidor
    let ssh_state_clone = ssh_state.clone();
    let ui_weak = ui.as_weak();
    ui.on_ssh_delete_server(move |server_id| {
        match ssh_state_clone
            .persistence
            .remove_server(&server_id.to_string())
        {
            Ok(_) => {
                if let Some(ui) = ui_weak.upgrade() {
                    ui.set_notification_message("Servidor removido com sucesso!".into());
                    ui.set_notification_is_error(false);
                    ui.set_show_notification(true);

                    // Recarregar lista
                    if let Ok(servers) = ssh_state_clone.load_servers() {
                        let current_id =
                            ssh_state_clone.get_current_server_id().unwrap_or_default();
                        let ui_servers: Vec<SshServerData> = servers
                            .into_iter()
                            .map(|server| {
                                let mut ui_data = convert_to_ui_data(server);
                                ui_data.is_connected = ui_data.id.as_str() == current_id;
                                ui_data
                            })
                            .collect();

                        ui.set_ssh_servers(ui_servers.as_slice().into());
                    }
                }
            }
            Err(e) => {
                if let Some(ui) = ui_weak.upgrade() {
                    ui.set_notification_message(std::format!("Erro ao remover: {}", e).into());
                    ui.set_notification_is_error(true);
                    ui.set_show_notification(true);
                }
            }
        }
    });

    // Callback para toggle favorito
    let ssh_state_clone = ssh_state.clone();
    let ui_weak = ui.as_weak();
    ui.on_ssh_toggle_favorite(move |server_id| {
        match ssh_state_clone
            .persistence
            .toggle_favorite(&server_id.to_string())
        {
            Ok(_) => {
                if let Some(ui) = ui_weak.upgrade() {
                    // Recarregar lista
                    if let Ok(servers) = ssh_state_clone.load_servers() {
                        let current_id =
                            ssh_state_clone.get_current_server_id().unwrap_or_default();
                        let ui_servers: Vec<SshServerData> = servers
                            .into_iter()
                            .map(|server| {
                                let mut ui_data = convert_to_ui_data(server);
                                ui_data.is_connected = ui_data.id.as_str() == current_id;
                                ui_data
                            })
                            .collect();

                        ui.set_ssh_servers(ui_servers.as_slice().into());
                    }
                }
            }
            Err(e) => {
                if let Some(ui) = ui_weak.upgrade() {
                    ui.set_notification_message(std::format!("Erro: {}", e).into());
                    ui.set_notification_is_error(true);
                    ui.set_show_notification(true);
                }
            }
        }
    });

    // Callback para atualizar lista
    let ssh_state_clone = ssh_state.clone();
    let ui_weak = ui.as_weak();
    ui.on_ssh_refresh_servers(move || {
        if let Some(ui) = ui_weak.upgrade() {
            if let Ok(servers) = ssh_state_clone.load_servers() {
                let current_id = ssh_state_clone.get_current_server_id().unwrap_or_default();
                let ui_servers: Vec<SshServerData> = servers
                    .into_iter()
                    .map(|server| {
                        let mut ui_data = convert_to_ui_data(server);
                        ui_data.is_connected = ui_data.id.as_str() == current_id;
                        ui_data
                    })
                    .collect();

                ui.set_ssh_servers(ui_servers.as_slice().into());
            }
        }
    });

    // Callback para testar conexão
    let _ssh_state_clone = ssh_state.clone();
    let ui_weak = ui.as_weak();
    ui.on_ssh_test_connection(move |server_data| {
        let ui_weak = ui_weak.clone();
        let connection = SshConnection {
            host: server_data.host.to_string(),
            port: server_data.port as u16,
            username: server_data.username.to_string(),
            password: server_data.password.to_string(),
            private_key: None,
            passphrase: None,
        };

        // Usar slint::invoke_from_event_loop em vez de tokio::spawn
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let mut test_manager = DockerManager::new();
                match test_manager.connect(connection).await {
                    Ok(_) => {
                        test_manager.disconnect();

                        // Usar invoke_from_event_loop para atualizar a UI na thread principal
                        let ui_weak_success = ui_weak.clone();
                        slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak_success.upgrade() {
                                ui.set_ssh_is_testing(false);
                                ui.set_notification_message("Conexão SSH testada com sucesso!".into());
                                ui.set_notification_is_error(false);
                                ui.set_show_notification(true);
                            } else {
                            }
                        }).unwrap();
                    }
                    Err(e) => {
                        let error_msg = std::format!("Falha no teste: {}", e);
                        let ui_weak_error = ui_weak.clone();

                        slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak_error.upgrade() {
                                ui.set_ssh_is_testing(false);
                                ui.set_notification_message(error_msg.into());
                                ui.set_notification_is_error(true);
                                ui.set_show_notification(true);
                            } else {
                            }
                        }).unwrap();
                    }
                }
            });
        });
    });
}
