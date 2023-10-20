use anyhow::Result;
use egui::{ColorImage, Ui};
use itertools::iproduct;
use rand::{distributions::Alphanumeric, Rng};
use sciimg::prelude::Image;
use solhat::ser::SerFile;
use solhat::ser::SerFrame;

use crate::analysis;
use crate::state::ApplicationState;

use egui_plot::{Legend, Line, LineStyle, Plot, PlotPoints};
use epaint::Color32;

#[derive(Default, Debug, Copy, Clone)]
struct Bin {
    count: u32,
}

#[derive(Default, Debug, Clone)]
struct Histogram {
    num_bins: usize,
    min_value: f32,
    max_value: f32,
    bins: Vec<Bin>,
}

impl Histogram {
    pub fn new(num_bins: usize, min_value: f32, max_value: f32) -> Self {
        Histogram {
            num_bins,
            min_value,
            max_value,
            bins: (0..num_bins).into_iter().map(|_| Bin::default()).collect(),
        }
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
            .map(|(i, b)| [i as f64, b.count as f64])
            .collect();

        Line::new(points)
            .color(Color32::LIGHT_BLUE)
            .style(LineStyle::Solid)
            .fill(0.0)
            .width(2.0)
    }
}

pub struct SerPreviewPane {
    texture_handle: Option<egui::TextureHandle>,
    texture_name: String,
    ser_file: Option<SerFile>,
    histogram: Option<Histogram>,
    show_frame_no: usize,
    animate: bool,
}

impl Default for SerPreviewPane {
    fn default() -> Self {
        Self {
            texture_handle: None,
            ser_file: None,
            texture_name: SerPreviewPane::gen_random_texture_name(),
            histogram: None,
            show_frame_no: 0,
            animate: false,
        }
    }
}

impl SerPreviewPane {
    pub fn is_empty(&self) -> bool {
        self.texture_handle.is_none()
    }

    // https://stackoverflow.com/questions/54275459/how-do-i-create-a-random-string-by-sampling-from-alphanumeric-characters
    fn gen_random_texture_name() -> String {
        rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(7)
            .map(char::from)
            .collect()
    }

    fn ser_frame_to_retained_image(ser_frame: &Image) -> ColorImage {
        let mut copied = ser_frame.clone();
        let size: [usize; 2] = [copied.width as _, copied.height as _];
        copied.normalize_to_8bit();
        let mut rgb: Vec<u8> = Vec::with_capacity(copied.height * copied.width * 3);
        iproduct!(0..copied.height, 0..copied.width).for_each(|(y, x)| {
            let (r, g, b) = if copied.num_bands() == 1 {
                (
                    copied.get_band(0).get(x, y),
                    copied.get_band(0).get(x, y),
                    copied.get_band(0).get(x, y),
                )
            } else {
                (
                    copied.get_band(0).get(x, y),
                    copied.get_band(1).get(x, y),
                    copied.get_band(2).get(x, y),
                )
            };
            rgb.push(r as u8);
            rgb.push(g as u8);
            rgb.push(b as u8);
        });
        ColorImage::from_rgb(size, &rgb)
    }

    fn update_texture(&mut self, ctx: &egui::Context) -> Result<()> {
        if let Some(ser_file) = &self.ser_file {
            let first_image: SerFrame = ser_file.get_frame(self.show_frame_no)?;
            let cimage = SerPreviewPane::ser_frame_to_retained_image(&first_image.buffer);
            self.texture_handle =
                Some(ctx.load_texture(&self.texture_name, cimage, Default::default()));
            Ok(())
        } else {
            Err(anyhow!("No ser file loaded"))
        }
    }

    fn update_histogram(&mut self) -> Result<()> {
        if let Some(ser_file) = &self.ser_file {
            let mut histogram = Histogram::new(1500, 0.0, 65536.0);
            histogram.compute_from_image(&ser_file.get_frame(self.show_frame_no)?.buffer);

            self.histogram = Some(histogram);
            Ok(())
        } else {
            Err(anyhow!("No ser file loaded"))
        }
    }

    pub fn load_ser(&mut self, ctx: &egui::Context, texture_path: &str) -> Result<()> {
        self.ser_file = Some(SerFile::load_ser(&texture_path)?);

        self.update_texture(ctx)?;
        self.update_histogram()?;

        Ok(())
    }

    pub fn unload_ser(&mut self) {
        self.texture_handle = None;
        self.ser_file = None;
        self.histogram = None;
    }

    pub fn threshold_test(&mut self, ui: &egui::Ui, state: &ApplicationState) -> Result<()> {
        if self.ser_file.is_some() {
            let result = analysis::threshold::run_thresh_test(&state.to_parameters())?;
            let cimage = SerPreviewPane::ser_frame_to_retained_image(&result);
            let texture = ui
                .ctx()
                .load_texture(&self.texture_name, cimage, Default::default());
            self.texture_handle = Some(texture);
            Ok(())
        } else {
            Err(anyhow!("Cannot perform threshtest: No image assigned"))
        }
    }

    pub fn size(&self) -> Result<[usize; 2]> {
        if let Some(texture_handle) = &self.texture_handle {
            Ok(texture_handle.size())
        } else {
            Err(anyhow!("Texture not loaded"))
        }
    }

    fn metadata_ui(&mut self, ui: &mut Ui) {
        if let Some(ser_file) = &self.ser_file {
            ui.horizontal(|ui| {
                ui.vertical_centered(|ui| {
                    ui.horizontal(|ui| {
                        ui.label("File:");
                        ui.label(format!("{}", ser_file.source_file));
                    });
                    egui::Grid::new("metadata")
                        .num_columns(2)
                        .spacing([40.0, 4.0])
                        .striped(true)
                        .show(ui, |ui| {
                            ui.label("Image Width:");
                            ui.label(format!("{}", ser_file.image_width));

                            ui.label("Image Height:");
                            ui.label(format!("{}", ser_file.image_height));
                            ui.end_row();

                            ui.label("Pixel Depth:");
                            ui.label(format!("{} bits", ser_file.pixel_depth));

                            ui.label("Frame Count:");
                            ui.label(format!("{}", ser_file.frame_count));
                            ui.end_row();

                            ui.label("Observer:");
                            ui.label(&ser_file.observer);

                            ui.label("Instrument:");
                            ui.label(&ser_file.instrument);
                            ui.end_row();

                            ui.label("Telescope:");
                            ui.label(format!("{}", ser_file.telescope));

                            ui.label("Time of Observation (UTC):");
                            ui.label(format!("{:?}", ser_file.date_time_utc.to_chrono_utc()));
                            ui.end_row();
                        });
                });
                let plot = Plot::new("histogram")
                    .legend(Legend::default())
                    .y_axis_width(4)
                    .show_axes(false)
                    .allow_scroll(false)
                    .allow_boxed_zoom(false)
                    .allow_drag(false)
                    .allow_zoom(false)
                    .show_grid(true);
                plot.show(ui, |plot_ui| {
                    if let Some(histogram) = &self.histogram {
                        plot_ui.line(histogram.to_line());
                    }
                });
            });
        }
    }
    fn options_ui(&mut self, ui: &mut Ui) -> Result<()> {
        self.update_histogram().unwrap();
        self.update_texture(ui.ctx()).unwrap();

        let Self {
            texture_handle: _,
            texture_name: _,
            ser_file,
            histogram: _,
            show_frame_no,
            animate,
        } = self;

        if let Some(ser_file) = &ser_file {
            // This is not a very efficient video viewer. Indeed, it's not written to be any good, just enough
            // to preview the frames in the file.
            if *animate {
                *show_frame_no += 1;
                if *show_frame_no == ser_file.frame_count {
                    *show_frame_no = 0;
                }
            }

            ui.horizontal(|ui| {
                if ui.button("<").clicked() {
                    if *show_frame_no > 0 {
                        *show_frame_no -= 1;
                    } else {
                        *show_frame_no = ser_file.frame_count - 1
                    }
                }

                if *animate {
                    if ui.button("⏸").clicked() {
                        *animate = false;
                    }
                } else {
                    if ui.button("⏵").clicked() {
                        *animate = true;
                    }
                }

                if ui.button("⏹").clicked() {
                    *animate = false;
                    *show_frame_no = 0;
                }
                if ui.button(">").clicked() {
                    if *show_frame_no < ser_file.frame_count - 1 {
                        *show_frame_no += 1;
                    } else {
                        *show_frame_no = 0;
                    }
                }
            });
            if ui
                .add(
                    egui::Slider::new(show_frame_no, 0..=(ser_file.frame_count - 1))
                        .prefix("Frame: "),
                )
                .changed()
            {
                self.update_histogram().unwrap();
                self.update_texture(ui.ctx()).unwrap();
            };
        }

        Ok(())
        // Add some options
    }
}

impl SerPreviewPane {
    pub fn ui(&mut self, ui: &mut Ui) {
        self.metadata_ui(ui);

        if let Some(texture_handle) = &self.texture_handle {
            ui.add(egui::Image::from_texture(texture_handle).shrink_to_fit());
        } else {
            ui.horizontal_centered(|ui| {
                ui.vertical_centered(|ui| {
                    ui.label("No image loaded");
                });
            });
        }

        self.options_ui(ui).unwrap();
    }
}
