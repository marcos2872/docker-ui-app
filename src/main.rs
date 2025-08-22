// Inclui módulos gerados pelo Slint
slint::include_modules!();

// Imports necessários para timer, interface e threading
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};

// Módulos locais
mod chart;
mod docker;
mod list_containers;
mod list_images;
mod list_networks;
mod list_volumes;
mod ssh;
mod ssh_persistence;
mod ssh_ui_integration;
mod ui;

// Tipos do Docker e gráficos
use chart::{ChartPoint, ChartRenderer};

use crate::ssh_ui_integration::{SshUiState, setup_ssh_ui};
use crate::ui::setup_global_callbacks;

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

// Dados dos gráficos para container específico
struct ContainerChartData {
    cpu_points: VecDeque<ChartPoint>,
    memory_points: VecDeque<ChartPoint>,
    last_update: Instant,
}

impl ContainerChartData {
    fn new() -> Self {
        Self {
            cpu_points: VecDeque::new(),
            memory_points: VecDeque::new(),
            last_update: Instant::now() - Duration::from_secs(2),
        }
    }

    fn should_update(&self) -> bool {
        self.last_update.elapsed().as_millis() >= 500 // 500ms entre atualizações (mesmo que dashboard)
    }

    fn add_cpu_point(&mut self, value: f32) {
        let time = chrono::Local::now().format("%H:%M:%S").to_string();
        self.cpu_points.push_back(ChartPoint { time, value });

        if self.cpu_points.len() > 60 {
            self.cpu_points.pop_front();
        }
        self.last_update = Instant::now();
    }

    fn add_memory_point(&mut self, value: f32) {
        let time = chrono::Local::now().format("%H:%M:%S").to_string();
        self.memory_points.push_back(ChartPoint { time, value });

        if self.memory_points.len() > 60 {
            self.memory_points.pop_front();
        }
        self.last_update = Instant::now();
    }

    fn clear(&mut self) {
        self.cpu_points.clear();
        self.memory_points.clear();
        self.last_update = Instant::now() - Duration::from_secs(2);
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

    // Configura renderizadores para gráficos de container específico (mesmo tamanho do dashboard)
    let mut container_cpu_chart_renderer = ChartRenderer::new(800, 256);
    container_cpu_chart_renderer.set_line_color([59, 130, 246]);

    let mut container_memory_chart_renderer = ChartRenderer::new(800, 256);
    container_memory_chart_renderer.set_line_color([16, 185, 129]);

    let app_state = AppState {
        chart_data: Arc::new(std::sync::Mutex::new(ChartData::new())),
        cpu_chart_renderer: Arc::new(std::sync::Mutex::new(cpu_chart_renderer)),
        memory_chart_renderer: Arc::new(std::sync::Mutex::new(memory_chart_renderer)),
    };

    // Dados e renderizadores para gráficos de container
    let container_chart_data = Arc::new(std::sync::Mutex::new(ContainerChartData::new()));
    let container_cpu_renderer = Arc::new(std::sync::Mutex::new(container_cpu_chart_renderer));
    let container_memory_renderer =
        Arc::new(std::sync::Mutex::new(container_memory_chart_renderer));

    // Configura callbacks globais da UI
    setup_global_callbacks(ui.as_weak(), app_state.clone());

    // Configura interface SSH
    let ssh_state = Arc::new(SshUiState::new().expect("Failed to initialize SSH state"));
    setup_ssh_ui(
        &ui,
        ssh_state,
        app_state,
        container_chart_data.clone(),
        container_cpu_renderer,
        container_memory_renderer,
    );

    // Configura callback para limpar dados ao mudar de container
    {
        let container_chart_data_clone = container_chart_data.clone();
        let ui_weak = ui.as_weak();

        ui.on_container_selected(move |_container_name| {
            // Limpa os dados de gráfico quando um novo container é selecionado
            if let Ok(mut chart_data) = container_chart_data_clone.try_lock() {
                chart_data.clear();
            }

            // Limpa dados da UI
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_container_cpu_usage("0.0%".into());
                ui.set_container_memory_usage("0 MB".into());
                ui.set_container_network_rx("0 B/s".into());
                ui.set_container_network_tx("0 B/s".into());
                ui.set_container_logs("".into());

                // Reset das configurações de logs e métricas
                ui.set_logs_expanded(false);
                ui.set_metrics_expanded(false);

                // Reset logs loading
                ui.set_logs_lines_loaded(50);
            }
        });
    }

    // Executa aplicação
    ui.run()
}
