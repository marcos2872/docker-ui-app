use crate::chart::ChartRenderer;
use crate::docker::{
    types::{ContainerInfo, CreateContainerRequest, DockerInfo, EnvVar, PortMapping, VolumeMapping},
    DockerManager, DockerManagement,
};
use crate::list_containers::{ContainerUIManager, SlintContainerData, setup_container_ui_timer};
use crate::list_images::{ImageUIManager, SlintImageData};
use crate::list_networks::{NetworkUIManager, SlintNetworkData};
use crate::list_volumes::{SlintVolumeData, VolumeUIManager};
use slint::{Timer, TimerMode, Weak};
use std::sync::Arc;
use std::time::Duration;

// Tipos gerados pelo Slint são importados diretamente
use crate::{
    AppState, AppWindow, ContainerChartData, ContainerData, ImageData, NetworkData, VolumeData,
};

pub struct UiApp {}

impl UiApp {
    pub fn new() -> Self {
        Self {}
    }
}

// Configura interface Docker e sistema de monitoramento
pub fn setup_docker_ui(
    ui_weak: Weak<AppWindow>,
    app_state: AppState,
    docker_manager: Arc<tokio::sync::Mutex<DockerManager>>,
    container_chart_data: Arc<std::sync::Mutex<ContainerChartData>>,
    container_cpu_renderer: Arc<std::sync::Mutex<ChartRenderer>>,
    container_memory_renderer: Arc<std::sync::Mutex<ChartRenderer>>,
) -> Timer {
    let _ui = ui_weak.upgrade().unwrap();
    let timer = Timer::default();

    let docker_manager_shared = docker_manager;

    let docker_manager_clone = docker_manager_shared.clone();
    let ui_weak_clone = ui_weak.clone();
    tokio::spawn(async move {
        let ui_weak_info = ui_weak_clone.clone();
        let docker_manager_info = docker_manager_clone.clone();
        // Carrega informações do Docker
        if let Ok(info) = docker_manager_info.lock().await.get_docker_info().await {
            slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_weak_info.upgrade() {
                    update_docker_info(&ui, &info);
                }
            })
            .unwrap();
        }

        let ui_weak_containers = ui_weak_clone.clone();
        let docker_manager_containers = docker_manager_clone.clone();
        // Carrega lista de containers
        if let Ok(containers) = docker_manager_containers
            .lock()
            .await
            .list_containers()
            .await
        {
            slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_weak_containers.upgrade() {
                    update_containers_list(&ui, &containers);
                    // Converte containers para formato Slint
                    let slint_containers: Vec<SlintContainerData> =
                        containers.iter().map(SlintContainerData::from).collect();
                    update_ui_containers_from_slint(&ui, &slint_containers);
                }
            })
            .unwrap();
        }
    });

    let ui_weak_timer = ui_weak.clone();
    let chart_data_timer = app_state.chart_data.clone();
    let cpu_chart_renderer_timer = app_state.cpu_chart_renderer.clone();
    let memory_chart_renderer_timer = app_state.memory_chart_renderer.clone();

    // Configura gerenciador de containers UI
    let container_ui_manager = Arc::new(tokio::sync::Mutex::new(ContainerUIManager::new(
        docker_manager_shared.clone(),
    )));

    let ui_weak_container = ui_weak.clone();
    let container_timer = setup_container_ui_timer(
        container_ui_manager.clone(),
        Arc::new(move |containers| {
            if let Some(ui) = ui_weak_container.upgrade() {
                update_ui_containers_from_slint(&ui, &containers);

                // Se estivermos na tela de detalhes, atualiza o container selecionado
                if ui.get_current_screen() == 6 {
                    let selected = ui.get_selected_container();
                    if !selected.name.is_empty() {
                        // Procura o container atualizado na lista
                        if let Some(updated_container) =
                            containers.iter().find(|c| c.name == selected.name)
                        {
                            // Cria um novo ContainerData com os dados atualizados
                            ui.set_selected_container(ContainerData {
                                name: updated_container.name.clone(),
                                image: updated_container.image.clone(),
                                status: updated_container.status.clone(),
                                ports: updated_container.ports.clone(),
                                running_for: updated_container.running_for.clone(),
                            });
                        }
                    }
                }
            }
        }),
    );

    // Mantém o timer vivo armazenando-o no contexto
    std::mem::forget(container_timer);

    // Configura callbacks de container
    setup_container_callbacks(ui_weak.clone(), container_ui_manager.clone());

    // Configura callback para carregar mais logs
    setup_load_more_logs_callback(ui_weak.clone(), docker_manager_shared.clone());

    // Configura callback para toggle de logs
    setup_toggle_logs_callback(ui_weak.clone());

    // Configura timer para logs de container
    setup_container_logs_timer(ui_weak.clone(), docker_manager_shared.clone());

    // Configura timer para stats de container
    setup_container_stats_timer(
        ui_weak.clone(),
        docker_manager_shared.clone(),
        container_chart_data,
        container_cpu_renderer,
        container_memory_renderer,
    );

    // Configura callbacks de criação de containers
    setup_create_container_callbacks(ui_weak.clone(), docker_manager_shared.clone());

    // Configura gerenciador de imagens UI
    let image_ui_manager = Arc::new(tokio::sync::Mutex::new(ImageUIManager::new(
        docker_manager_shared.clone(),
    )));

    // Configura callbacks de imagem
    setup_image_callbacks(ui_weak.clone(), image_ui_manager.clone());

    // Configura timer para atualizar imagens a cada segundo
    let ui_weak_images = ui_weak.clone();
    let image_ui_manager_timer = image_ui_manager.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(1));

        loop {
            interval.tick().await;

            let mut manager = image_ui_manager_timer.lock().await;
            match manager.refresh_images().await {
                Ok(()) => {
                    let images = manager.get_images();
                    let ui_weak_clone = ui_weak_images.clone();
                    slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_weak_clone.upgrade() {
                            ui.set_image_list_error("".into());
                            update_ui_images_from_slint(&ui, &images);
                        }
                    })
                    .unwrap();
                }
                Err(e) => {
                    let error_message = e.to_string();
                    let ui_weak_clone = ui_weak_images.clone();
                    slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_weak_clone.upgrade() {
                            ui.set_image_list_error(error_message.into());
                        }
                    })
                    .unwrap();
                }
            }
        }
    });

    // Configura gerenciador de networks UI
    let network_ui_manager = Arc::new(tokio::sync::Mutex::new(NetworkUIManager::new(
        docker_manager_shared.clone(),
    )));

    // Configura callbacks de network
    setup_network_callbacks(ui_weak.clone(), network_ui_manager.clone());

    // Configura timer para atualizar networks a cada segundo
    let ui_weak_networks = ui_weak.clone();
    let network_ui_manager_timer = network_ui_manager.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(1));

        loop {
            interval.tick().await;

            let mut manager = network_ui_manager_timer.lock().await;
            match manager.refresh_networks().await {
                Ok(()) => {
                    let networks = manager.get_networks();
                    let ui_weak_clone = ui_weak_networks.clone();
                    slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_weak_clone.upgrade() {
                            ui.set_network_list_error("".into());
                            update_ui_networks_from_slint(&ui, &networks);
                        }
                    })
                    .unwrap();
                }
                Err(e) => {
                    let error_message = e.to_string();
                    let ui_weak_clone = ui_weak_networks.clone();
                    slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_weak_clone.upgrade() {
                            ui.set_network_list_error(error_message.into());
                        }
                    })
                    .unwrap();
                }
            }
        }
    });

    // Configura gerenciador de volumes UI
    let volume_ui_manager = Arc::new(tokio::sync::Mutex::new(VolumeUIManager::new(
        docker_manager_shared.clone(),
    )));

    // Configura callbacks de volume
    setup_volume_callbacks(ui_weak.clone(), volume_ui_manager.clone());

    // Configura timer para atualizar volumes a cada segundo
    let ui_weak_volumes = ui_weak.clone();
    let volume_ui_manager_timer = volume_ui_manager.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(1));

        loop {
            interval.tick().await;

            let mut manager = volume_ui_manager_timer.lock().await;
            match manager.refresh_volumes().await {
                Ok(()) => {
                    let volumes = manager.get_volumes();
                    let ui_weak_clone = ui_weak_volumes.clone();
                    slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_weak_clone.upgrade() {
                            ui.set_volume_list_error("".into());
                            update_ui_volumes_from_slint(&ui, &volumes);
                        }
                    })
                    .unwrap();
                }
                Err(e) => {
                    let error_message = e.to_string();
                    let ui_weak_clone = ui_weak_volumes.clone();
                    slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_weak_clone.upgrade() {
                            ui.set_volume_list_error(error_message.into());
                        }
                    })
                    .unwrap();
                }
            }
        }
    });

    // Inicialização manual dos containers
    let ui_weak_init = ui_weak.clone();
    let container_ui_manager_init = container_ui_manager.clone();
    tokio::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        {
            let mut manager = container_ui_manager_init.lock().await;
            if let Ok(()) = manager.refresh_containers().await {
                let filtered_containers = manager.get_filtered_containers();
                slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_weak_init.upgrade() {
                        update_ui_containers_from_slint(&ui, &filtered_containers);
                    }
                })
                .unwrap();
            }
        }
    });

    // Timer para atualizar estatísticas a cada segundo
    timer.start(TimerMode::Repeated, Duration::from_secs(1), move || {
        let ui_weak_clone = ui_weak_timer.clone();
        let ui_weak_clone2 = ui_weak_timer.clone();
        let chart_data_clone = chart_data_timer.clone();
        let cpu_chart_renderer_clone = cpu_chart_renderer_timer.clone();
        let memory_chart_renderer_clone = memory_chart_renderer_timer.clone();
        let docker_manager_clone = docker_manager_shared.clone();
        let docker_manager_clone2 = docker_manager_shared.clone();

        // Task para informações gerais do Docker
        tokio::spawn(async move {
            let docker_manager = docker_manager_clone2.lock().await;
            if let Ok(info) = docker_manager.get_docker_info().await {
                slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_weak_clone2.upgrade() {
                        update_docker_info(&ui, &info);
                    }
                })
                .unwrap();
            }
        });

        // Task principal para estatísticas do sistema - USANDO A MESMA INSTÂNCIA
        tokio::spawn(async move {
            let mut docker_manager = docker_manager_clone.lock().await;
            match docker_manager.get_docker_system_usage().await {
                Ok(stats) => {
                    // Atualiza UI no thread principal
                    slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_weak_clone.upgrade() {
                            ui.set_cpu_usage_str(
                                format!(
                                    "{:.2}% | {}%",
                                    stats.cpu_usage,
                                    stats.cpu_online * 100
                                )
                                .into(),
                            );
                            ui.set_memory_percentage_str(
                                format_memory_display(
                                    stats.memory_percentage,
                                    stats.memory_usage,
                                    stats.memory_limit,
                                )
                                .into(),
                            );
                            ui.set_network_rx_str(
                                format!("RX {}", format_bytes(stats.network_rx_bytes))
                                    .into(),
                            );
                            ui.set_network_tx_str(
                                format!("TX {}", format_bytes(stats.network_tx_bytes))
                                    .into(),
                            );

                            // Atualiza dados dos gráficos com throttling adequado
                            if let Ok(mut chart_data_lock) = chart_data_clone.lock() {
                                if chart_data_lock.should_update() {
                                    chart_data_lock.add_cpu_point(stats.cpu_usage as f32);
                                    chart_data_lock
                                        .add_memory_point(stats.memory_percentage as f32);

                                    // Renderiza gráfico CPU
                                    let cpu_chart_renderer =
                                        cpu_chart_renderer_clone.lock().unwrap();
                                    let cpu_chart = cpu_chart_renderer.render_line_chart(
                                        &chart_data_lock.cpu_points.make_contiguous(),
                                        stats.cpu_online as f32 * 100.0,
                                    );
                                    ui.set_cpu_chart(cpu_chart);

                                    // Renderiza gráfico memória
                                    let memory_chart_renderer =
                                        memory_chart_renderer_clone.lock().unwrap();
                                    let memory_chart = memory_chart_renderer
                                        .render_line_chart(
                                            &chart_data_lock
                                                .memory_points
                                                .make_contiguous(),
                                            100.0,
                                        );
                                    ui.set_memory_chart(memory_chart);
                                }
                            }
                        }
                    })
                    .unwrap();
                }
                Err(e) => {
                            let error_message = format!("Error getting docker stats: {}", e);
                            eprintln!("{}", error_message);
                            slint::invoke_from_event_loop(move || {
                                if let Some(ui) = ui_weak_clone.upgrade() {
                                    ui.set_notification_message(error_message.into());
                                    ui.set_notification_is_error(true);
                                    ui.set_show_notification(true);
                                }
                            })
                            .unwrap();
                }
            }
        });
    });

    timer
}

// Atualiza informações do Docker na interface
fn update_docker_info(ui: &AppWindow, info: &DockerInfo) {
    ui.set_total_containers(info.containers as i32);
    ui.set_running_containers(info.containers_running as i32);
    ui.set_stopped_containers(info.containers_stopped as i32);
    ui.set_paused_containers(info.containers_paused as i32);
    ui.set_total_images(info.images as i32);
    ui.set_docker_version(format!("{} | {}", info.version, info.architecture).into());
}

// Atualiza lista de containers (não implementado)
fn update_containers_list(_ui: &AppWindow, _containers: &[ContainerInfo]) {
    // Funcionalidade não implementada ainda
}

// Converte imagens para formato Slint e atualiza UI
fn update_ui_images_from_slint(ui: &AppWindow, images: &[SlintImageData]) {
    let slint_images: Vec<_> = images
        .iter()
        .map(|image| ImageData {
            id: image.id.clone(),
            tag: image.tag.clone(),
            size: image.size.clone(),
            created: image.created.clone(),
            in_use: image.in_use,
        })
        .collect();

    let slint_model: std::rc::Rc<slint::VecModel<ImageData>> =
        std::rc::Rc::new(slint::VecModel::from(slint_images));

    ui.set_images(slint_model.into());
}

// Converte networks para formato Slint e atualiza UI
fn update_ui_networks_from_slint(ui: &AppWindow, networks: &[SlintNetworkData]) {
    let slint_networks: Vec<_> = networks
        .iter()
        .map(|network| NetworkData {
            id: network.id.clone(),
            name: network.name.clone(),
            driver: network.driver.clone(),
            scope: network.scope.clone(),
            created: network.created.clone(),
            containers_count: network.containers_count,
            is_system: network.is_system,
        })
        .collect();

    let slint_model: std::rc::Rc<slint::VecModel<NetworkData>> =
        std::rc::Rc::new(slint::VecModel::from(slint_networks));

    ui.set_networks(slint_model.into());
}

// Converte volumes para formato Slint e atualiza UI
fn update_ui_volumes_from_slint(ui: &AppWindow, volumes: &[SlintVolumeData]) {
    let slint_volumes: Vec<_> = volumes
        .iter()
        .map(|volume| VolumeData {
            name: volume.name.clone(),
            driver: volume.driver.clone(),
            mountpoint: volume.mountpoint.clone(),
            created: volume.created.clone(),
            containers_count: volume.containers_count,
        })
        .collect();

    let slint_model: std::rc::Rc<slint::VecModel<VolumeData>> =
        std::rc::Rc::new(slint::VecModel::from(slint_volumes));

    ui.set_volumes(slint_model.into());
}

// Converte containers para formato Slint e atualiza UI
fn update_ui_containers_from_slint(ui: &AppWindow, containers: &[SlintContainerData]) {
    let slint_containers: Vec<_> = containers
        .iter()
        .map(|container| ContainerData {
            name: container.name.clone(),
            image: container.image.clone(),
            status: container.status.clone(),
            ports: container.ports.clone(),
            running_for: container.running_for.clone(),
        })
        .collect();

    let slint_model: std::rc::Rc<slint::VecModel<ContainerData>> =
        std::rc::Rc::new(slint::VecModel::from(slint_containers));

    ui.set_containers(slint_model.into());
}

// Configura callbacks específicos para containers
fn setup_container_callbacks(
    ui_weak: Weak<AppWindow>,
    container_ui_manager: Arc<tokio::sync::Mutex<ContainerUIManager>>,
) {
    let ui = ui_weak.upgrade().unwrap();

    // Callback para mudança na busca de containers
    ui.on_search_changed({
        let ui_weak = ui_weak.clone();
        let container_manager = container_ui_manager.clone();
        move |search_text| {
            let ui_weak_clone = ui_weak.clone();
            let container_manager_clone = container_manager.clone();
            let search_string = search_text.to_string();

            tokio::spawn(async move {
                let mut manager = container_manager_clone.lock().await;
                manager.set_search_filter(search_string);
                let filtered_containers = manager.get_filtered_containers();
                slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_weak_clone.upgrade() {
                        update_ui_containers_from_slint(&ui, &filtered_containers);
                    }
                })
                .unwrap();
            });
        }
    });

    // Callback para mudança no filtro de status
    ui.on_filter_changed({
        let ui_weak = ui_weak.clone();
        let container_manager = container_ui_manager.clone();
        move |status_filter| {
            let ui_weak_clone = ui_weak.clone();
            let container_manager_clone = container_manager.clone();
            let status_string = status_filter.to_string();

            tokio::spawn(async move {
                let mut manager = container_manager_clone.lock().await;
                manager.set_status_filter(status_string);
                let filtered_containers = manager.get_filtered_containers();
                slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_weak_clone.upgrade() {
                        update_ui_containers_from_slint(&ui, &filtered_containers);
                    }
                })
                .unwrap();
            });
        }
    });

    // Callback para ações em containers
    ui.on_container_action({
        let ui_weak = ui_weak.clone();
        let container_manager = container_ui_manager.clone();
        move |container_name, action| {
            let ui_weak_clone = ui_weak.clone();
            let container_manager_clone = container_manager.clone();
            let container_name_str = container_name.to_string();
            let action_str = action.to_string();
            let loading_key = format!("{}_{}", container_name_str, action_str);

            tokio::spawn(async move {
                // Define o estado de loading
                let ui_weak_loading = ui_weak_clone.clone();
                let loading_key_clone = loading_key.clone();
                slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_weak_loading.upgrade() {
                        ui.set_container_loading(loading_key_clone.into());
                        ui.set_container_error("".into());
                        ui.set_container_success("".into());
                    }
                })
                .unwrap();

                let (success, error_message) = {
                    let manager = container_manager_clone.lock().await;

                    // Executa a ação no container
                    match manager
                        .execute_container_action(&container_name_str, &action_str)
                        .await
                    {
                        Ok(()) => (true, None),
                        Err(e) => (false, Some(e.to_string())),
                    }
                };

                // Limpa o loading e trata resultado
                let ui_weak_result = ui_weak_clone.clone();
                if success {
                    let success_msg = match action_str.as_str() {
                        "start" => {
                            format!("Container '{}' iniciado com sucesso", container_name_str)
                        }
                        "stop" => format!("Container '{}' parado com sucesso", container_name_str),
                        "pause" => {
                            format!("Container '{}' pausado com sucesso", container_name_str)
                        }
                        "unpause" => {
                            format!("Container '{}' despausado com sucesso", container_name_str)
                        }
                        "remove" => {
                            format!("Container '{}' removido com sucesso", container_name_str)
                        }
                        _ => format!(
                            "Ação '{}' executada com sucesso no container '{}'",
                            action_str, container_name_str
                        ),
                    };

                    slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_weak_result.upgrade() {
                            ui.set_container_loading("".into());
                            ui.set_notification_message(success_msg.into());
                            ui.set_notification_is_error(false);
                            ui.set_show_notification(true);
                        }
                    })
                    .unwrap();

                    // Timer para limpar mensagem de sucesso após 3 segundos
                    let ui_weak_timer = ui_weak_clone.clone();
                    tokio::spawn(async move {
                        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
                        slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak_timer.upgrade() {
                                ui.set_show_notification(false);
                            }
                        })
                        .unwrap();
                    });
                } else if let Some(error) = error_message {
                    let error_msg = format!(
                        "Erro ao executar '{}' no container '{}': {}",
                        action_str, container_name_str, error
                    );
                    slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_weak_result.upgrade() {
                            ui.set_container_loading("".into());
                            ui.set_notification_message(error_msg.into());
                            ui.set_notification_is_error(true);
                            ui.set_show_notification(true);
                        }
                    })
                    .unwrap();
                }

                // Atualiza a lista imediatamente após a ação bem-sucedida
                if success {
                    let mut manager = container_manager_clone.lock().await;
                    if let Ok(()) = manager.refresh_containers().await {
                        let filtered_containers = manager.get_filtered_containers();
                        let ui_weak_final = ui_weak_clone.clone();
                        slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak_final.upgrade() {
                                update_ui_containers_from_slint(&ui, &filtered_containers);
                            }
                        })
                        .unwrap();
                    }
                }
            });
        }
    });
}

// Configura callbacks específicos para imagens
fn setup_image_callbacks(
    ui_weak: Weak<AppWindow>,
    image_ui_manager: Arc<tokio::sync::Mutex<ImageUIManager>>,
) {
    let ui = ui_weak.upgrade().unwrap();

    // Callback para refresh de imagens
    ui.on_refresh_images_clicked({
        let ui_weak = ui_weak.clone();
        let image_manager = image_ui_manager.clone();
        move || {
            let ui_weak_clone = ui_weak.clone();
            let image_manager_clone = image_manager.clone();

            tokio::spawn(async move {
                let mut manager = image_manager_clone.lock().await;
                match manager.refresh_images().await {
                    Ok(()) => {
                        let images = manager.get_images();
                        slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak_clone.upgrade() {
                                ui.set_image_list_error("".into());
                                update_ui_images_from_slint(&ui, &images);
                            }
                        })
                        .unwrap();
                    }
                    Err(e) => {
                        let error_message = e.to_string();
                        slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak_clone.upgrade() {
                                ui.set_image_list_error(error_message.into());
                            }
                        })
                        .unwrap();
                    }
                }
            });
        }
    });

    // Callback para ações em imagens
    ui.on_image_action({
        let ui_weak = ui_weak.clone();
        let image_manager = image_ui_manager.clone();
        move |image_id, action| {
            let ui_weak_clone = ui_weak.clone();
            let image_manager_clone = image_manager.clone();
            let image_id_str = image_id.to_string();
            let action_str = action.to_string();
            let loading_key = format!("{}_{}", image_id_str, action_str);

            tokio::spawn(async move {
                // Define o estado de loading
                let ui_weak_loading = ui_weak_clone.clone();
                let loading_key_clone = loading_key.clone();
                slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_weak_loading.upgrade() {
                        ui.set_image_loading(loading_key_clone.into());
                    }
                })
                .unwrap();

                let manager = image_manager_clone.lock().await;

                // Executa a ação na imagem
                let result = manager
                    .execute_image_action(&image_id_str, &action_str)
                    .await;

                let ui_weak_result = ui_weak_clone.clone();
                match result {
                    Ok(success_message) => {
                        slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak_result.upgrade() {
                                ui.set_image_loading("".into());
                                ui.set_notification_message(success_message.into());
                                ui.set_notification_is_error(false);
                                ui.set_show_notification(true);
                            }
                        })
                        .unwrap();

                        // Timer para limpar mensagem de sucesso após 3 segundos
                        let ui_weak_timer = ui_weak_clone.clone();
                        tokio::spawn(async move {
                            tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
                            slint::invoke_from_event_loop(move || {
                                if let Some(ui) = ui_weak_timer.upgrade() {
                                    ui.set_show_notification(false);
                                }
                            })
                            .unwrap();
                        });
                    }
                    Err(error_message) => {
                        let error_message_clone = error_message.clone();
                        slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak_result.upgrade() {
                                ui.set_image_loading("".into());
                                let formatted_error = if error_message_clone.starts_with("IN_USE:")
                                {
                                    error_message_clone
                                        .strip_prefix("IN_USE:")
                                        .unwrap_or(&error_message_clone)
                                        .to_string()
                                } else {
                                    error_message_clone
                                        .strip_prefix("OTHER_ERROR:")
                                        .unwrap_or(&error_message_clone)
                                        .to_string()
                                };
                                ui.set_notification_message(formatted_error.into());
                                ui.set_notification_is_error(true);
                                ui.set_show_notification(true);
                            }
                        })
                        .unwrap();
                    }
                }

                // Atualiza a lista imediatamente após a ação
                drop(manager); // Libera o lock
                let mut manager = image_manager_clone.lock().await;
                if let Ok(()) = manager.refresh_images().await {
                    let images = manager.get_images();
                    let ui_weak_final = ui_weak_clone.clone();
                    slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_weak_final.upgrade() {
                            update_ui_images_from_slint(&ui, &images);
                        }
                    })
                    .unwrap();
                }
            });
        }
    });
}

// Configura callbacks específicos para networks
fn setup_network_callbacks(
    ui_weak: Weak<AppWindow>,
    network_ui_manager: Arc<tokio::sync::Mutex<NetworkUIManager>>,
) {
    let ui = ui_weak.upgrade().unwrap();

    // Callback para refresh de networks
    ui.on_refresh_networks_clicked({
        let ui_weak = ui_weak.clone();
        let network_manager = network_ui_manager.clone();
        move || {
            let ui_weak_clone = ui_weak.clone();
            let network_manager_clone = network_manager.clone();

            tokio::spawn(async move {
                let mut manager = network_manager_clone.lock().await;
                match manager.refresh_networks().await {
                    Ok(()) => {
                        let networks = manager.get_networks();
                        slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak_clone.upgrade() {
                                ui.set_network_list_error("".into());
                                update_ui_networks_from_slint(&ui, &networks);
                            }
                        })
                        .unwrap();
                    }
                    Err(e) => {
                        let error_message = e.to_string();
                        slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak_clone.upgrade() {
                                ui.set_network_list_error(error_message.into());
                            }
                        })
                        .unwrap();
                    }
                }
            });
        }
    });

    // Callback para ações em networks
    ui.on_network_action({
        let ui_weak = ui_weak.clone();
        let network_manager = network_ui_manager.clone();
        move |network_id, action| {
            let ui_weak_clone = ui_weak.clone();
            let network_manager_clone = network_manager.clone();
            let network_id_str = network_id.to_string();
            let action_str = action.to_string();
            let loading_key = format!("{}_{}", network_id_str, action_str);

            tokio::spawn(async move {
                // Define o estado de loading
                let ui_weak_loading = ui_weak_clone.clone();
                let loading_key_clone = loading_key.clone();
                slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_weak_loading.upgrade() {
                        ui.set_network_loading(loading_key_clone.into());
                    }
                })
                .unwrap();

                let manager = network_manager_clone.lock().await;

                // Executa a ação na network
                let result = manager
                    .execute_network_action(&network_id_str, &action_str)
                    .await;

                let ui_weak_result = ui_weak_clone.clone();
                match result {
                    Ok(success_message) => {
                        slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak_result.upgrade() {
                                ui.set_network_loading("".into());
                                ui.set_notification_message(success_message.into());
                                ui.set_notification_is_error(false);
                                ui.set_show_notification(true);
                            }
                        })
                        .unwrap();

                        // Timer para limpar mensagem de sucesso após 3 segundos
                        let ui_weak_timer = ui_weak_clone.clone();
                        tokio::spawn(async move {
                            tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
                            slint::invoke_from_event_loop(move || {
                                if let Some(ui) = ui_weak_timer.upgrade() {
                                    ui.set_show_notification(false);
                                }
                            })
                            .unwrap();
                        });
                    }
                    Err(error_message) => {
                        let error_message_clone = error_message.clone();
                        slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak_result.upgrade() {
                                ui.set_network_loading("".into());
                                let formatted_error = if error_message_clone.starts_with("IN_USE:")
                                {
                                    error_message_clone
                                        .strip_prefix("IN_USE:")
                                        .unwrap_or(&error_message_clone)
                                        .to_string()
                                } else {
                                    error_message_clone
                                        .strip_prefix("OTHER_ERROR:")
                                        .unwrap_or(&error_message_clone)
                                        .to_string()
                                };
                                ui.set_notification_message(formatted_error.into());
                                ui.set_notification_is_error(true);
                                ui.set_show_notification(true);
                            }
                        })
                        .unwrap();
                    }
                }

                // Atualiza a lista imediatamente após a ação
                drop(manager); // Libera o lock
                let mut manager = network_manager_clone.lock().await;
                if let Ok(()) = manager.refresh_networks().await {
                    let networks = manager.get_networks();
                    let ui_weak_final = ui_weak_clone.clone();
                    slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_weak_final.upgrade() {
                            update_ui_networks_from_slint(&ui, &networks);
                        }
                    })
                    .unwrap();
                }
            });
        }
    });
}

// Configura callbacks específicos para volumes
fn setup_volume_callbacks(
    ui_weak: Weak<AppWindow>,
    volume_ui_manager: Arc<tokio::sync::Mutex<VolumeUIManager>>,
) {
    let ui = ui_weak.upgrade().unwrap();

    // Callback para refresh de volumes
    ui.on_refresh_volumes_clicked({
        let ui_weak = ui_weak.clone();
        let volume_manager = volume_ui_manager.clone();
        move || {
            let ui_weak_clone = ui_weak.clone();
            let volume_manager_clone = volume_manager.clone();

            tokio::spawn(async move {
                let mut manager = volume_manager_clone.lock().await;
                match manager.refresh_volumes().await {
                    Ok(()) => {
                        let volumes = manager.get_volumes();
                        slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak_clone.upgrade() {
                                ui.set_volume_list_error("".into());
                                update_ui_volumes_from_slint(&ui, &volumes);
                            }
                        })
                        .unwrap();
                    }
                    Err(e) => {
                        let error_message = e.to_string();
                        slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak_clone.upgrade() {
                                ui.set_volume_list_error(error_message.into());
                            }
                        })
                        .unwrap();
                    }
                }
            });
        }
    });

    // Callback para ações em volumes
    ui.on_volume_action({
        let ui_weak = ui_weak.clone();
        let volume_manager = volume_ui_manager.clone();
        move |volume_name, action| {
            let ui_weak_clone = ui_weak.clone();
            let volume_manager_clone = volume_manager.clone();
            let volume_name_str = volume_name.to_string();
            let action_str = action.to_string();
            let loading_key = format!("{}_{}", volume_name_str, action_str);

            tokio::spawn(async move {
                // Define o estado de loading
                let ui_weak_loading = ui_weak_clone.clone();
                let loading_key_clone = loading_key.clone();
                slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_weak_loading.upgrade() {
                        ui.set_volume_loading(loading_key_clone.into());
                    }
                })
                .unwrap();

                let manager = volume_manager_clone.lock().await;

                // Executa a ação no volume
                let result = manager
                    .execute_volume_action(&volume_name_str, &action_str)
                    .await;

                let ui_weak_result = ui_weak_clone.clone();
                match result {
                    Ok(success_message) => {
                        slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak_result.upgrade() {
                                ui.set_volume_loading("".into());
                                ui.set_notification_message(success_message.into());
                                ui.set_notification_is_error(false);
                                ui.set_show_notification(true);
                            }
                        })
                        .unwrap();

                        // Timer para limpar mensagem de sucesso após 3 segundos
                        let ui_weak_timer = ui_weak_clone.clone();
                        tokio::spawn(async move {
                            tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
                            slint::invoke_from_event_loop(move || {
                                if let Some(ui) = ui_weak_timer.upgrade() {
                                    ui.set_show_notification(false);
                                }
                            })
                            .unwrap();
                        });
                    }
                    Err(error_message) => {
                        let error_message_clone = error_message.clone();
                        slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak_result.upgrade() {
                                ui.set_volume_loading("".into());
                                let formatted_error = if error_message_clone.starts_with("IN_USE:")
                                {
                                    error_message_clone
                                        .strip_prefix("IN_USE:")
                                        .unwrap_or(&error_message_clone)
                                        .to_string()
                                } else {
                                    error_message_clone
                                        .strip_prefix("OTHER_ERROR:")
                                        .unwrap_or(&error_message_clone)
                                        .to_string()
                                };
                                ui.set_notification_message(formatted_error.into());
                                ui.set_notification_is_error(true);
                                ui.set_show_notification(true);
                            }
                        })
                        .unwrap();
                    }
                }

                // Atualiza a lista imediatamente após a ação
                drop(manager); // Libera o lock
                let mut manager = volume_manager_clone.lock().await;
                if let Ok(()) = manager.refresh_volumes().await {
                    let volumes = manager.get_volumes();
                    let ui_weak_final = ui_weak_clone.clone();
                    slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_weak_final.upgrade() {
                            update_ui_volumes_from_slint(&ui, &volumes);
                        }
                    })
                    .unwrap();
                }
            });
        }
    });
}

// Configura callbacks da interface
pub fn setup_global_callbacks(ui_weak: Weak<AppWindow>, _app_state: AppState) {
    let ui = ui_weak.upgrade().unwrap();

    // Callback para mudança de tela
    ui.on_screen_changed({
        let ui_weak = ui_weak.clone();
        move |screen_index| {
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_current_screen(screen_index);
            }
        }
    });
}

// Configura callbacks para criação de containers
fn setup_create_container_callbacks(
    ui_weak: Weak<AppWindow>,
    docker_manager: Arc<tokio::sync::Mutex<DockerManager>>,
) {
    let ui = ui_weak.upgrade().unwrap();

    // Callback para criar container
    ui.on_create_container({
        let ui_weak = ui_weak.clone();
        let docker_manager = docker_manager.clone();
        move |name, image, command, restart_policy, ports_text, volumes_text, env_vars_text| {
            let ui_weak_clone = ui_weak.clone();
            let docker_manager_clone = docker_manager.clone();
            let name_str = name.to_string();
            let image_str = image.to_string();
            let command_str = command.to_string();
            let restart_policy_str = restart_policy.to_string();
            let ports_str = ports_text.to_string();
            let volumes_str = volumes_text.to_string();
            let env_vars_str = env_vars_text.to_string();

            tokio::spawn(async move {
                // Define estado de loading
                let ui_weak_loading = ui_weak_clone.clone();
                slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_weak_loading.upgrade() {
                        ui.set_creating_container(true);
                    }
                })
                .unwrap();

                // Valida campos obrigatórios
                if name_str.trim().is_empty() {
                    let ui_weak_error = ui_weak_clone.clone();
                    slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_weak_error.upgrade() {
                            ui.set_creating_container(false);
                            ui.set_notification_message("Nome do container é obrigatório".into());
                            ui.set_notification_is_error(true);
                            ui.set_show_notification(true);
                        }
                    })
                    .unwrap();
                    return;
                }

                if image_str.trim().is_empty() {
                    let ui_weak_error = ui_weak_clone.clone();
                    slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_weak_error.upgrade() {
                            ui.set_creating_container(false);
                            ui.set_notification_message("Nome da imagem é obrigatório".into());
                            ui.set_notification_is_error(true);
                            ui.set_show_notification(true);
                        }
                    })
                    .unwrap();
                    return;
                }

                // Parse dos campos de entrada
                let ports = parse_ports_text(&ports_str);
                let volumes = parse_volumes_text(&volumes_str);
                let env_vars = parse_env_vars_text(&env_vars_str);

                let create_request = CreateContainerRequest {
                    name: name_str.trim().to_string(),
                    image: image_str.trim().to_string(),
                    ports,
                    volumes,
                    environment: env_vars,
                    command: if command_str.trim().is_empty() {
                        None
                    } else {
                        Some(command_str.trim().to_string())
                    },
                    restart_policy: restart_policy_str,
                };

                // Executa criação
                let docker_manager = docker_manager_clone.lock().await;
                let result = docker_manager.create_container(create_request).await;

                match result {
                    Ok(container_id) => {
                        let ui_weak_success = ui_weak_clone.clone();
                        let container_name = name_str.clone();
                        slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak_success.upgrade() {
                                ui.set_creating_container(false);
                                ui.set_notification_message(
                                    format!(
                                        "Container '{}' criado e iniciado com sucesso!\nID: {}",
                                        container_name,
                                        &container_id[..12]
                                    )
                                    .into(),
                                );
                                ui.set_notification_is_error(false);
                                ui.set_show_notification(true);

                                // Agenda fechamento do modal e notificação juntos após 3 segundos
                                let ui_weak_timer = ui_weak_clone.clone();
                                tokio::spawn(async move {
                                    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                                    slint::invoke_from_event_loop(move || {
                                        if let Some(ui) = ui_weak_timer.upgrade() {
                                            // Fecha modal e notificação juntos
                                            ui.set_show_create_modal(false);
                                            ui.set_show_notification(false);

                                            // Limpa os campos
                                            ui.set_create_container_name("".into());
                                            ui.set_create_image_name("".into());
                                            ui.set_create_command("".into());
                                            ui.set_create_restart_policy("no".into());
                                            ui.set_create_ports_text("".into());
                                            ui.set_create_volumes_text("".into());
                                            ui.set_create_env_vars_text("".into());
                                        }
                                    })
                                    .unwrap();
                                });
                            }
                        })
                        .unwrap();
                    }
                    Err(e) => {
                        let error_message = e.to_string();
                        let ui_weak_error = ui_weak_clone.clone();
                        slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak_error.upgrade() {
                                ui.set_creating_container(false);
                                ui.set_notification_message(
                                    format!("Falha ao criar container:\n{}", error_message).into(),
                                );
                                ui.set_notification_is_error(true);
                                ui.set_show_notification(true);
                            }
                        })
                        .unwrap();
                    }
                }
            });
        }
    });

    // Callback para cancelar criação
    ui.on_cancel_create_container({
        let ui_weak = ui_weak.clone();
        move || {
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_show_create_modal(false);
                ui.set_create_container_name("".into());
                ui.set_create_image_name("".into());
                ui.set_create_command("".into());
                ui.set_create_restart_policy("no".into());
                ui.set_create_ports_text("".into());
                ui.set_create_volumes_text("".into());
                ui.set_create_env_vars_text("".into());
                ui.set_creating_container(false);
            }
        }
    });
}

// Formata bytes em unidades legíveis
fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

// Formata exibição de memória com porcentagem
fn format_memory_display(percentage: f64, usage: u64, limit: u64) -> String {
    const MB: u64 = 1024 * 1024;
    const GB: u64 = 1024 * MB;

    if usage >= GB {
        format!(
            "{:.2}% ({:.2} GB | {:.2} GB)",
            percentage,
            usage as f64 / GB as f64,
            limit as f64 / GB as f64
        )
    } else if usage < GB && limit >= GB {
        format!(
            "{:.2}% ({:.2} MB | {:.2} GB)",
            percentage,
            usage as f64 / MB as f64,
            limit as f64 / GB as f64
        )
    } else {
        format!(
            "{:.2}% ({:.2} MB | {:.2} MB)",
            percentage,
            usage as f64 / MB as f64,
            limit as f64 / MB as f64
        )
    }
}

// Configura timer para atualizar logs do container selecionado
fn setup_container_logs_timer(
    ui_weak: Weak<AppWindow>,
    docker_manager: Arc<tokio::sync::Mutex<DockerManager>>,
) {
    let timer = Timer::default();

    timer.start(TimerMode::Repeated, Duration::from_secs(1), move || {
        let ui_weak_clone = ui_weak.clone();
        let docker_manager_clone = docker_manager.clone();

        // Coleta as informações necessárias antes do tokio::spawn
        let (current_screen, container_name, lines_loaded) =
            if let Some(ui) = ui_weak_clone.upgrade() {
                let screen = ui.get_current_screen();
                let selected = ui.get_selected_container();
                let lines = ui.get_logs_lines_loaded();
                (screen, selected.name.to_string(), lines)
            } else {
                return; // Se não conseguir fazer upgrade, sai
            };

        // Só busca logs se estivermos na tela de detalhes (tela 6)
        if current_screen == 6 && !container_name.is_empty() {
            tokio::spawn(async move {
                let manager = docker_manager_clone.lock().await;

                // Usa o número de linhas já carregadas
                let tail_lines = if lines_loaded > 50 {
                    Some(lines_loaded as usize)
                } else {
                    None
                };

                match manager
                    .get_container_logs(&container_name, tail_lines)
                    .await
                {
                    Ok(logs) => {
                        slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak_clone.upgrade() {
                                ui.set_container_logs(logs.into());
                            }
                        })
                        .unwrap();
                    }
                    Err(_) => {
                        // Ignora erros de logs para não poluir interface
                    }
                }
            });
        }
    });

    // Mantém o timer vivo
    std::mem::forget(timer);
}

// Funções auxiliares para parsing de entrada

// Parse do texto de portas: "8080:80/tcp,9000:9000/udp"
fn parse_ports_text(ports_text: &str) -> Vec<PortMapping> {
    if ports_text.trim().is_empty() {
        return Vec::new();
    }

    ports_text
        .split(',')
        .filter_map(|port_str| {
            let port_str = port_str.trim();
            if port_str.is_empty() {
                return None;
            }

            // Separa protocolo se especificado
            let (port_part, protocol) = if port_str.contains('/') {
                let parts: Vec<&str> = port_str.split('/').collect();
                (parts[0], parts.get(1).unwrap_or(&"tcp").to_string())
            } else {
                (port_str, "tcp".to_string())
            };

            // Parse host:container
            let parts: Vec<&str> = port_part.split(':').collect();
            if parts.len() != 2 {
                return None;
            }

            let host_port = parts[0].parse::<u16>().ok()?;
            let container_port = parts[1].parse::<u16>().ok()?;

            Some(PortMapping {
                host_port,
                container_port,
                protocol,
            })
        })
        .collect()
}

// Parse do texto de volumes: "/host/path:/container/path,/host2:/container2:ro"
fn parse_volumes_text(volumes_text: &str) -> Vec<VolumeMapping> {
    if volumes_text.trim().is_empty() {
        return Vec::new();
    }

    volumes_text
        .split(',')
        .filter_map(|volume_str| {
            let volume_str = volume_str.trim();
            if volume_str.is_empty() {
                return None;
            }

            let parts: Vec<&str> = volume_str.split(':').collect();
            if parts.len() < 2 {
                return None;
            }

            let host_path = parts[0].to_string();
            let container_path = parts[1].to_string();
            let read_only = parts.get(2).map_or(false, |&mode| mode == "ro");

            Some(VolumeMapping {
                host_path,
                container_path,
                read_only,
            })
        })
        .collect()
}

// Parse do texto de variáveis de ambiente: "KEY1=value1,KEY2=value2"
fn parse_env_vars_text(env_vars_text: &str) -> Vec<EnvVar> {
    if env_vars_text.trim().is_empty() {
        return Vec::new();
    }

    env_vars_text
        .split(',')
        .filter_map(|env_str| {
            let env_str = env_str.trim();
            if env_str.is_empty() {
                return None;
            }

            let parts: Vec<&str> = env_str.splitn(2, '=').collect();
            if parts.len() != 2 {
                return None;
            }

            Some(EnvVar {
                key: parts[0].to_string(),
                value: parts[1].to_string(),
            })
        })
        .collect()
}

// Configura callback para carregar mais logs
fn setup_load_more_logs_callback(
    ui_weak: Weak<AppWindow>,
    docker_manager: Arc<tokio::sync::Mutex<DockerManager>>,
) {
    if let Some(ui) = ui_weak.upgrade() {
        ui.on_load_more_logs(move || {
            let ui_weak_clone = ui_weak.clone();
            let docker_manager_clone = docker_manager.clone();

            // Pega as informações antes do spawn
            let (container_name, current_lines) = if let Some(ui) = ui_weak_clone.upgrade() {
                let selected = ui.get_selected_container();
                let lines = ui.get_logs_lines_loaded();
                (selected.name.to_string(), lines)
            } else {
                return;
            };

            if container_name.is_empty() {
                return;
            }

            // Define loading state
            if let Some(ui) = ui_weak_clone.upgrade() {
                ui.set_logs_loading(true);
            }

            // Incrementa o número de linhas a buscar
            let new_lines_count = current_lines + 50;

            tokio::spawn(async move {
                let manager = docker_manager_clone.lock().await;

                // Busca mais 50 linhas
                match manager
                    .get_container_logs(&container_name, Some(new_lines_count as usize))
                    .await
                {
                    Ok(new_logs) => {
                        slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak_clone.upgrade() {
                                ui.set_container_logs(new_logs.into());
                                ui.set_logs_lines_loaded(new_lines_count);
                                ui.set_logs_loading(false);
                            }
                        })
                        .unwrap();
                    }
                    Err(_) => {
                        slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak_clone.upgrade() {
                                ui.set_logs_loading(false);
                            }
                        })
                        .unwrap();
                    }
                }
            });
        });
    }
}

// Configura callback para toggle de logs
fn setup_toggle_logs_callback(ui_weak: Weak<AppWindow>) {
    if let Some(ui) = ui_weak.upgrade() {
        ui.on_toggle_logs(move || {
            if let Some(ui) = ui_weak.upgrade() {
                let current_state = ui.get_logs_expanded();
                ui.set_logs_expanded(!current_state);
            }
        });
    }
}

// Configura timer para atualizar stats do container selecionado
fn setup_container_stats_timer(
    ui_weak: Weak<AppWindow>,
    docker_manager: Arc<tokio::sync::Mutex<DockerManager>>,
    container_chart_data: Arc<std::sync::Mutex<ContainerChartData>>,
    container_cpu_renderer: Arc<std::sync::Mutex<ChartRenderer>>,
    container_memory_renderer: Arc<std::sync::Mutex<ChartRenderer>>,
) {
    let timer = Timer::default();

    timer.start(TimerMode::Repeated, Duration::from_secs(1), move || {
        let ui_weak_clone = ui_weak.clone();
        let docker_manager_clone = docker_manager.clone();
        let chart_data_clone = container_chart_data.clone();
        let cpu_renderer_clone = container_cpu_renderer.clone();
        let memory_renderer_clone = container_memory_renderer.clone();

        // Coleta as informações necessárias antes do tokio::spawn
        let (current_screen, container_name) = if let Some(ui) = ui_weak_clone.upgrade() {
            let screen = ui.get_current_screen();
            let selected = ui.get_selected_container();
            (screen, selected.name.to_string())
        } else {
            return; // Se não conseguir fazer upgrade, sai
        };

        // Só busca stats se estivermos na tela de detalhes (tela 6) e container em execução
        if current_screen == 6 && !container_name.is_empty() {
            tokio::spawn(async move {
                let mut manager = docker_manager_clone.lock().await;

                match manager.get_single_container_stats(&container_name).await {
                    Ok((cpu, cpu_total, memory, rx, tx)) => {
                        // Extrai percentual de memória do string
                        let memory_percentage = memory
                            .split('%')
                            .next()
                            .and_then(|s| s.parse::<f32>().ok())
                            .unwrap_or(0.0);

                        // Atualiza dados dos gráficos
                        if let Ok(mut chart_data) = chart_data_clone.try_lock() {
                            if chart_data.should_update() {
                                chart_data.add_cpu_point(cpu as f32);
                                chart_data.add_memory_point(memory_percentage);
                            }
                        }

                        slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak_clone.upgrade() {
                                ui.set_container_cpu_usage(format!("{:.1}%", cpu).into());
                                ui.set_container_cpu_total(format!("{}%", cpu_total * 100).into());
                                ui.set_container_memory_usage(memory.into());
                                ui.set_container_network_rx(rx.into());
                                ui.set_container_network_tx(tx.into());

                                // Gera gráficos dentro do event loop para evitar problemas de threading
                                if let (Ok(mut chart_data), Ok(renderer)) =
                                    (chart_data_clone.try_lock(), cpu_renderer_clone.try_lock())
                                {
                                    let cpu_chart = renderer.render_line_chart(
                                        &chart_data.cpu_points.make_contiguous(),
                                        100.0,
                                    );
                                    ui.set_container_cpu_chart(cpu_chart);
                                }

                                if let (Ok(mut chart_data), Ok(renderer)) = (
                                    chart_data_clone.try_lock(),
                                    memory_renderer_clone.try_lock(),
                                ) {
                                    let memory_chart = renderer.render_line_chart(
                                        &chart_data.memory_points.make_contiguous(),
                                        100.0,
                                    );
                                    ui.set_container_memory_chart(memory_chart);
                                }
                            }
                        })
                        .unwrap();
                    }
                    Err(_) => {
                        // Container pode estar parado, define valores padrão e gráficos vazios
                        slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak_clone.upgrade() {
                                ui.set_container_cpu_usage("0.0%".into());
                                ui.set_container_cpu_total("0%".into());
                                ui.set_container_memory_usage("N/A".into());
                                ui.set_container_network_rx("0 B/s".into());
                                ui.set_container_network_tx("0 B/s".into());
                                
                                // Gera gráficos vazios para containers parados
                                if let (Ok(renderer), Ok(memory_renderer)) = (
                                    cpu_renderer_clone.try_lock(),
                                    memory_renderer_clone.try_lock(),
                                ) {
                                    let empty_data = vec![];
                                    let cpu_chart = renderer.render_line_chart(&empty_data, 100.0);
                                    let memory_chart = memory_renderer.render_line_chart(&empty_data, 100.0);
                                    ui.set_container_cpu_chart(cpu_chart);
                                    ui.set_container_memory_chart(memory_chart);
                                }
                            }
                        })
                        .unwrap();
                    }
                }
            });
        }
    });

    // Mantém o timer vivo
    std::mem::forget(timer);
}
