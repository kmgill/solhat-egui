use crate::analysis;
use crate::imageutil;
use crate::state::ApplicationState;
use anyhow::Error;
use anyhow::Result;
use egui::Ui;
use solhat::ser::SerFile;
use solhat::ser::SerFrame;
// use std::{error::Error, fmt};
use crate::histogram::Histogram;

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
            texture_name: imageutil::gen_random_texture_name(),
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

    fn update_texture(&mut self, ctx: &egui::Context) -> Result<()> {
        if let Some(ser_file) = &self.ser_file {
            let first_image: SerFrame = ser_file.get_frame(self.show_frame_no)?;
            let cimage = imageutil::sciimg_to_color_image(&first_image.buffer);
            self.texture_handle =
                Some(ctx.load_texture(&self.texture_name, cimage, Default::default()));
            Ok(())
        } else {
            Err(Error::msg("No ser file loaded"))
        }
    }

    fn update_histogram(&mut self) -> Result<()> {
        if let Some(ser_file) = &self.ser_file {
            let mut histogram = Histogram::new(1500, 0.0, 65536.0);
            histogram.compute_from_image(&ser_file.get_frame(self.show_frame_no)?.buffer);

            self.histogram = Some(histogram);
            Ok(())
        } else {
            Err(Error::msg("No ser file loaded"))
        }
    }

    pub fn load_ser(&mut self, ctx: &egui::Context, texture_path: &str) -> Result<()> {
        self.ser_file = Some(SerFile::load_ser(texture_path)?);

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
            let cimage = imageutil::sciimg_to_color_image(&result);
            let texture = ui
                .ctx()
                .load_texture(&self.texture_name, cimage, Default::default());
            self.texture_handle = Some(texture);
            Ok(())
        } else {
            Err(Error::msg("Cannot perform threshtest: No image assigned"))
        }
    }

    pub fn size(&self) -> Result<[usize; 2]> {
        if let Some(texture_handle) = &self.texture_handle {
            Ok(texture_handle.size())
        } else {
            Err(Error::msg("Texture not loaded"))
        }
    }

    fn metadata_ui(&mut self, ui: &mut Ui) {
        if let Some(ser_file) = &self.ser_file {
            ui.horizontal(|ui| {
                ui.vertical_centered(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(t!("preview.file"));
                        ui.label(ser_file.source_file.to_string());
                    });
                    egui::Grid::new("metadata")
                        .num_columns(2)
                        .spacing([40.0, 4.0])
                        .striped(true)
                        .show(ui, |ui| {
                            ui.label(t!("preview.image_width"));
                            ui.label(ser_file.image_width.to_string());

                            ui.label(t!("preview.image_height"));
                            ui.label(ser_file.image_height.to_string());
                            ui.end_row();

                            ui.label(t!("preview.pixel_depth"));
                            ui.label(format!("{} {}", ser_file.pixel_depth, t!("preview.bits")));

                            ui.label(t!("preview.frame_count"));
                            ui.label(ser_file.frame_count.to_string());
                            ui.end_row();

                            ui.label(t!("preview.observer"));
                            ui.label(&ser_file.observer);

                            ui.label(t!("preview.instrument"));
                            ui.label(&ser_file.instrument);
                            ui.end_row();

                            ui.label(t!("preview.telescope"));
                            ui.label(ser_file.telescope.to_string());

                            ui.label(t!("preview.time_of_observation"));
                            ui.label(format!("{:?}", ser_file.date_time_utc.to_chrono_utc()));
                            ui.end_row();
                        });
                });

                if let Some(histogram) = &mut self.histogram {
                    histogram.ui(ui);
                }
            });
        }
    }
    fn options_ui(&mut self, ui: &mut Ui) -> Result<()> {
        if self.animate {
            self.update_histogram().unwrap();
            self.update_texture(ui.ctx()).unwrap();
        }

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
                } else if ui.button("⏵").clicked() {
                    *animate = true;
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
                        .prefix(t!("preview.frame")),
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
