use anyhow::Result;
use egui::{ColorImage, Ui};
use itertools::iproduct;
use rand::{distributions::Alphanumeric, Rng};
use sciimg::prelude::Image;
use solhat::ser::SerFile;
use solhat::ser::SerFrame;

use crate::analysis;
use crate::state::ApplicationState;

pub struct SerPreviewPane {
    texture_handle: Option<egui::TextureHandle>,
    texture_name: String,
    ser_file: Option<SerFile>,
}

impl Default for SerPreviewPane {
    fn default() -> Self {
        Self {
            texture_handle: None,
            ser_file: None,
            texture_name: SerPreviewPane::gen_random_texture_name(),
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

    fn load_ser_texture(
        &self,
        ctx: &egui::Context,
        ser_file: &SerFile,
    ) -> Result<egui::TextureHandle> {
        let first_image: SerFrame = ser_file.get_frame(0)?;
        let cimage = SerPreviewPane::ser_frame_to_retained_image(&first_image.buffer);
        Ok(ctx.load_texture(&self.texture_name, cimage, Default::default()))
    }

    pub fn load_ser(&mut self, ctx: &egui::Context, texture_path: &str) -> Result<()> {
        let ser_file = SerFile::load_ser(&texture_path)?;
        self.texture_handle = Some(self.load_ser_texture(ctx, &ser_file)?);
        self.ser_file = Some(ser_file);
        Ok(())
    }

    pub fn unload_ser(&mut self) {
        self.texture_handle = None;
        self.ser_file = None;
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
        }
    }
    fn options_ui(&mut self, _ui: &mut Ui) {

        // Add some options
    }
}

impl SerPreviewPane {
    pub fn ui(&mut self, ui: &mut Ui) {
        self.metadata_ui(ui);
        self.options_ui(ui);

        if let Some(texture_handle) = &self.texture_handle {
            ui.add(egui::Image::from_texture(texture_handle).shrink_to_fit());
        } else {
            ui.horizontal_centered(|ui| {
                ui.vertical_centered(|ui| {
                    ui.label("No image loaded");
                });
            });
        }
    }
}
