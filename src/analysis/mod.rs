use egui::{Response, Ui};

use egui_plot::{Legend, Line, LineStyle, Plot, PlotPoints};
use epaint::Color32;

#[allow(dead_code)]
pub mod sigma;
pub mod threshold;

#[derive(Clone)]
pub struct AnalysisChart {
    pub data: sigma::AnalysisSeries,
    sma_period: usize,
    show_axes: bool,
    show_grid: bool,
}

impl Default for AnalysisChart {
    fn default() -> Self {
        AnalysisChart {
            data: Default::default(),
            sma_period: 5,
            show_axes: true,
            show_grid: true,
        }
    }
}

impl AnalysisChart {
    #[allow(dead_code)]
    pub fn new(data: sigma::AnalysisSeries) -> Self {
        AnalysisChart {
            data,
            sma_period: 5,
            show_axes: true,
            show_grid: true,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.data.sigma_list.is_empty()
    }

    fn raw_data_line(&self) -> Line {
        let raw_list_points: PlotPoints = self
            .data
            .sigma_list
            .clone()
            .into_iter()
            .enumerate()
            .map(|(i, v)| [i as f64, v])
            .collect();

        Line::new(raw_list_points)
            .color(Color32::from_rgb(50, 50, 50))
            .style(LineStyle::Solid)
            .name("Raw Values")
    }

    fn sorted_data_line(&self) -> Line {
        let sorted_list_points: PlotPoints = self
            .data
            .sorted_list()
            .into_iter()
            .enumerate()
            .map(|(i, v)| [i as f64, v])
            .collect();

        Line::new(sorted_list_points)
            .color(Color32::from_rgb(100, 200, 100))
            .style(LineStyle::Solid)
            .name("Sorted")
    }

    fn sma_line(&self) -> Line {
        let sma_list_points: PlotPoints = self
            .data
            .sma(self.sma_period)
            .into_iter()
            .enumerate()
            .map(|(i, v)| [i as f64, v])
            .collect();

        Line::new(sma_list_points)
            .color(Color32::LIGHT_BLUE)
            .style(LineStyle::Solid)
            .width(2.0)
            .name(format!("SMA({})", self.sma_period))
    }

    fn options_ui(&mut self, ui: &mut Ui) {
        let Self {
            data,
            sma_period,
            show_axes,
            show_grid,
        } = self;
        ui.horizontal(|ui| {
            ui.label("SMA Period:");
            ui.add(
                egui::DragValue::new(sma_period)
                    .speed(1.0)
                    .clamp_range(2.0..=data.sigma_list.len() as f64)
                    .prefix("p: "),
            );
            ui.checkbox(show_axes, "Show axes");
            ui.checkbox(show_grid, "Show grid");
        });
    }
}

impl AnalysisChart {
    pub fn ui(&mut self, ui: &mut Ui) -> Response {
        self.options_ui(ui);

        let Self {
            data: _,
            sma_period: _,
            show_axes,
            show_grid,
        } = self;

        let plot = Plot::new("data_analysis")
            .legend(Legend::default())
            .y_axis_width(4)
            .show_axes(*show_axes)
            .show_grid(*show_grid);
        plot.show(ui, |plot_ui| {
            plot_ui.line(self.raw_data_line());
            plot_ui.line(self.sorted_data_line());
            plot_ui.line(self.sma_line());
        })
        .response
    }
}
