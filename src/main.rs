// Inclui módulos gerados pelo Slint
slint::include_modules!();

// Imports necessários para timer, interface e threading
use slint::{Timer, TimerMode, ToSharedString, Weak};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};

// Módulos locais
mod chart;
mod docker;

// Tipos do Docker e gráficos
use chart::{ChartPoint, ChartRenderer};
use docker::{ContainerInfo, DockerInfo, DockerManager};

// Struct Container para interface Slint
// #[derive(Clone)]
// struct Container {
//     id: slint::SharedString,
//     name: slint::SharedString,
//     image: slint::SharedString,
//     state: slint::SharedString,
//     status: slint::SharedString,
// }

// Estado global da aplicação
#[derive(Clone)]
struct AppState {
    chart_data: Arc<std::sync::Mutex<ChartData>>,
    cpu_chart_renderer: Arc<std::sync::Mutex<ChartRenderer>>,
    memory_chart_renderer: Arc<std::sync::Mutex<ChartRenderer>>,
}

// Dados dos gráficos em tempo real
struct ChartData {
    cpu_points: VecDeque<ChartPoint>,
    memory_points: VecDeque<ChartPoint>,
    last_update: Instant,
}

impl ChartData {
    fn new() -> Self {
        Self {
            cpu_points: VecDeque::new(),
            memory_points: VecDeque::new(),
            last_update: Instant::now() - Duration::from_secs(2), // Força primeira atualização
        }
    }

    // Verifica se precisa atualizar (reduzido para melhor sincronia com docker stats)
    fn should_update(&self) -> bool {
        self.last_update.elapsed().as_millis() >= 500 // 500ms entre atualizações
    }

    // Adiciona ponto de CPU (max 60 pontos)
    fn add_cpu_point(&mut self, value: f32) {
        let time = chrono::Local::now().format("%H:%M:%S").to_string();
        self.cpu_points.push_back(ChartPoint { time, value });

        if self.cpu_points.len() > 60 {
            self.cpu_points.pop_front();
        }
        self.last_update = Instant::now();
    }

    // Adiciona ponto de memória (max 60 pontos)
    fn add_memory_point(&mut self, value: f32) {
        let time = chrono::Local::now().format("%H:%M:%S").to_string();
        self.memory_points.push_back(ChartPoint { time, value });

        if self.memory_points.len() > 60 {
            self.memory_points.pop_front();
        }
        self.last_update = Instant::now();
    }
}

// Função principal assíncrona
#[tokio::main]
async fn main() -> Result<(), slint::PlatformError> {
    // Cria janela da aplicação
    let ui = AppWindow::new()?;

    // Configura renderizador de gráfico CPU (azul)
    let mut cpu_chart_renderer = ChartRenderer::new(800, 256);
    cpu_chart_renderer.set_line_color([59, 130, 246]);

    // Configura renderizador de gráfico memória (verde)
    let mut memory_chart_renderer = ChartRenderer::new(800, 256);
    memory_chart_renderer.set_line_color([16, 185, 129]);

    let app_state = AppState {
        chart_data: Arc::new(std::sync::Mutex::new(ChartData::new())),
        cpu_chart_renderer: Arc::new(std::sync::Mutex::new(cpu_chart_renderer)),
        memory_chart_renderer: Arc::new(std::sync::Mutex::new(memory_chart_renderer)),
    };

    // Configura interface Docker e inicia timer
    let _timer = setup_docker_ui(ui.as_weak(), app_state).await;

    // Executa aplicação
    ui.run()
}

// Configura interface Docker e sistema de monitoramento
async fn setup_docker_ui(ui_weak: Weak<AppWindow>, app_state: AppState) -> Timer {
    let ui = ui_weak.upgrade().unwrap();

    let timer = Timer::default();
    // Verifica se Docker está rodando
    match DockerManager::new().await {
        Ok(docker_manager) => {
            ui.set_docker_status("Verificando...".into());

            let docker_status = docker_manager.check_docker_status();
            ui.set_docker_status(docker_status.to_shared_string());

            // Carrega informações do Docker
            if let Ok(info) = docker_manager.get_docker_info().await {
                update_docker_info(&ui, &info);
            }

            // Carrega lista de containers
            if let Ok(containers) = docker_manager.list_containers().await {
                update_containers_list(&ui, &containers);
            }

            let ui_weak_timer = ui_weak.clone();
            let chart_data_timer = app_state.chart_data.clone();
            let cpu_chart_renderer_timer = app_state.cpu_chart_renderer.clone();
            let memory_chart_renderer_timer = app_state.memory_chart_renderer.clone();

            // Cria uma única instância do DockerManager compartilhada entre atualizações
            let docker_manager_shared = Arc::new(tokio::sync::Mutex::new(docker_manager));

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
                            eprintln!("Error getting docker stats: {}", e);
                        }
                    }
                });
            });
        }
        Err(_) => {
            ui.set_docker_status("NotRunning".into());
        }
    }

    // Configura callbacks da interface
    setup_callbacks(ui_weak, app_state.clone());

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

// Configura callbacks da interface
fn setup_callbacks(ui_weak: Weak<AppWindow>, _app_state: AppState) {
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
