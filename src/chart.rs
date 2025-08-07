// Renderização de gráficos para interface Slint
use plotters::prelude::*;
use slint::{Image, Rgb8Pixel, SharedPixelBuffer};

// Renderizador de gráficos com configurações customizáveis
pub struct ChartRenderer {
    width: u32,
    height: u32,
    line_color: [u8; 3],
}

// Ponto de dados para o gráfico
#[derive(Debug, Clone)]
pub struct ChartPoint {
    pub time: String,
    pub value: f32,
}

impl ChartRenderer {
    // Cria novo renderizador com cor padrão azul
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            line_color: [59, 130, 246], // #3b82f6 - blue
        }
    }

    // Define cor da linha do gráfico
    pub fn set_line_color(&mut self, color: [u8; 3]) {
        self.line_color = color;
    }

    // Renderiza gráfico de linha e retorna imagem
    pub fn render_line_chart(&self, data: &[ChartPoint], max_percentage: f32) -> Image {
        // Buffer RGB para a imagem
        let mut buffer = vec![0u8; (self.width * self.height * 3) as usize];

        // Cria backend de desenho
        {
            let backend = BitMapBackend::with_buffer(&mut buffer, (self.width, self.height))
                .into_drawing_area();

            // Preenche fundo com tema escuro
            backend.fill(&RGBColor(46, 48, 48)).unwrap(); // #2e3030

            if !data.is_empty() {
                // Encontra valores mínimo e máximo para eixo Y
                let min_value = data.iter().map(|p| p.value).fold(f32::INFINITY, f32::min);
                let max_value = data
                    .iter()
                    .map(|p| p.value)
                    .fold(f32::NEG_INFINITY, f32::max);

                // Calcula padding apropriado para o intervalo
                let value_range = max_value - min_value;
                let padding = {
                    // Calcula o próximo múltiplo de 10
                    let mut rounded_value = (value_range / 10.0).ceil() * 10.0;

                    // Garante que o padding não exceda o valor máximo
                    if rounded_value > max_percentage {
                        rounded_value = max_percentage;
                    }

                    // Lida com os casos de valores menores
                    if rounded_value < 10.0 && rounded_value > 5.0 {
                        10.0
                    } else if rounded_value <= 5.0 {
                        5.0
                    } else {
                        rounded_value
                    }
                };
                let y_min = 0.0;
                let y_max = padding;

                // Cria contexto do gráfico
                let mut chart = ChartBuilder::on(&backend)
                    .margin(5)
                    .x_label_area_size(20)
                    .y_label_area_size(40)
                    .build_cartesian_2d(0f32..(data.len() as f32).max(1.0), y_min..y_max)
                    .unwrap();

                // Configura estilo do gráfico com tema escuro
                chart
                    .configure_mesh()
                    .x_desc("")
                    .y_desc("Valor %")
                    .x_label_formatter(&|x| {
                        let index = *x as usize;
                        if index < data.len() {
                            data[index].time.clone()
                        } else {
                            String::new()
                        }
                    })
                    .axis_style(&RGBColor(107, 114, 128)) // #6b7280 - gray
                    .bold_line_style(&RGBColor(107, 114, 128).mix(0.3))
                    .light_line_style(&RGBColor(107, 114, 128).mix(0.1))
                    .label_style(("sans-serif", 12).into_font().color(&WHITE))
                    .y_max_light_lines(4)
                    .x_max_light_lines(6)
                    .draw()
                    .unwrap();

                // Desenha série de linha
                let line_color =
                    RGBColor(self.line_color[0], self.line_color[1], self.line_color[2]);

                // Desenha a linha principal
                chart
                    .draw_series(LineSeries::new(
                        data.iter()
                            .enumerate()
                            .map(|(i, point)| (i as f32, point.value)),
                        line_color.stroke_width(2),
                    ))
                    .unwrap();

                // Desenha pontos como círculos
                // chart
                //     .draw_series(data.iter().enumerate().map(|(i, point)| {
                //         Circle::new((i as f32, point.value), 3, line_color.filled())
                //     }))
                //     .unwrap();

                // Preenche área sob a curva com transparência
                chart
                    .draw_series(AreaSeries::new(
                        data.iter()
                            .enumerate()
                            .map(|(i, point)| (i as f32, point.value)),
                        y_min,
                        &line_color.mix(0.1),
                    ))
                    .unwrap();
            }

            // Finaliza desenho
            backend.present().unwrap();
        }

        // Converte buffer para imagem Slint
        let shared_buffer =
            SharedPixelBuffer::<Rgb8Pixel>::clone_from_slice(&buffer, self.width, self.height);
        Image::from_rgb8(shared_buffer)
    }
}
