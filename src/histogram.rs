use egui::Ui;
use egui_plot::{Legend, Line, LineStyle, Plot, PlotPoints};
use epaint::Color32;
use itertools::iproduct;
use sciimg::prelude::Image;

#[derive(Default, Debug, Copy, Clone)]
struct Bin {
    count: u32,
}

#[derive(Default, Debug, Clone)]
pub struct Histogram {
    num_bins: usize,
    min_value: f32,
    max_value: f32,
    bins: Vec<Bin>,
    pub logarithmic: bool,
}

impl Histogram {
    pub fn new(num_bins: usize, min_value: f32, max_value: f32) -> Self {
        Histogram {
            num_bins,
            min_value,
            max_value,
            logarithmic: false,
            bins: (0..num_bins).map(|_| Bin::default()).collect(),
        }
    }

    pub fn reset(&mut self) {
        self.bins = (0..self.num_bins).map(|_| Bin::default()).collect();
    }

    fn value_to_bin(&self, v: f32) -> usize {
        (self.num_bins as f32 * ((v - self.min_value) / (self.max_value - self.min_value))).floor()
            as usize
    }

    pub fn compute_from_image(&mut self, img: &Image) {
        iproduct!(0..img.height, 0..img.width).for_each(|(y, x)| {
            let v = img.get_band(0).get(x, y);
            let bin_no = self.value_to_bin(v);
            self.bins[bin_no].count += 1;
        });
    }

    pub fn to_line(&self) -> Line {
        let points: PlotPoints = self
            .bins
            .clone()
            .into_iter()
            .enumerate()
            .map(|(i, b)| {
                if self.logarithmic && b.count > 0 {
                    [i as f64, (b.count as f64).log10()]
                } else {
                    [i as f64, b.count as f64]
                }
            })
            .collect();

        Line::new(points)
            .color(Color32::LIGHT_BLUE)
            .style(LineStyle::Solid)
            .fill(0.0)
            .width(2.0)
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let plot = Plot::new("histogram")
            .legend(Legend::default())
            .y_axis_width(4)
            .show_axes(false)
            .auto_bounds_y()
            .allow_scroll(false)
            .allow_boxed_zoom(false)
            .allow_drag(false)
            .allow_zoom(false)
            .show_grid(true);
        plot.show(ui, |plot_ui| {
            plot_ui.line(self.to_line());
        })
        .response
        .context_menu(|ui| {
            if ui
                .checkbox(&mut self.logarithmic, t!("histogram.logarithmic"))
                .clicked()
            {
                ui.close_menu();
            }
        });
    }
}
