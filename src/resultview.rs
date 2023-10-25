use std::path::PathBuf;

use crate::histogram::Histogram;
use crate::imageutil;
use crate::process::RunResultsContainer;
use anyhow::Result;
use egui::Ui;

pub struct ResultViewPane {
    texture_handle: Option<egui::TextureHandle>,
    texture_name: String,
    results: Option<RunResultsContainer>,
    histogram: Histogram,
    exposure: f64,
    gamma: f64,
}

impl Default for ResultViewPane {
    fn default() -> Self {
        Self {
            texture_handle: None,
            texture_name: imageutil::gen_random_texture_name(),
            results: None,
            histogram: Histogram::new(1500, 0.0, 65536.0),
            exposure: 0.0,
            gamma: 1.0,
        }
    }
}

impl ResultViewPane {
    pub fn is_empty(&self) -> bool {
        self.texture_handle.is_none()
    }

    fn update_histogram(&mut self) -> Result<()> {
        self.histogram.reset();
        if let Some(results) = &self.results {
            self.histogram.compute_from_image(&results.image);
            Ok(())
        } else {
            Err(anyhow!("No ser file loaded"))
        }
    }

    fn update_texture(&mut self, ctx: &egui::Context) -> Result<()> {
        if let Some(results) = &self.results {
            let mut image_adjusted = results.image.clone();

            image_adjusted.levels_with_gamma(0.0, 1.0 - self.exposure as f32, self.gamma as f32);
            let cimage = imageutil::sciimg_to_color_image(&image_adjusted);
            self.texture_handle =
                Some(ctx.load_texture(&self.texture_name, cimage, Default::default()));
            Ok(())
        } else {
            Err(anyhow!("No ser file loaded"))
        }
    }

    pub fn set_image(&mut self, results: &RunResultsContainer, ctx: &egui::Context) -> Result<()> {
        self.results = Some(results.clone());
        self.update_texture(ctx)?;
        self.update_histogram()?;

        Ok(())
    }

    fn options_ui(&mut self, ui: &mut Ui) -> Result<()> {
        ui.horizontal(|ui| {
            ui.vertical_centered(|ui| {
                if let Some(results) = &self.results {
                    ui.horizontal(|ui| {
                        ui.label(t!("results.output_filename"));

                        ui.label(results.output_filename.to_string_lossy().as_ref());
                    });
                }

                egui::Grid::new("metadata")
                    .num_columns(2)
                    .spacing([40.0, 4.0])
                    .striped(true)
                    .show(ui, |ui| {
                        if let Some(results) = &self.results {
                            ui.label(t!("results.num_images_used"));
                            ui.label(results.num_frames_used.to_string());
                            ui.end_row();
                        }

                        ui.label(t!("results.exposure"));
                        if ui
                            .add(egui::Slider::new(&mut self.exposure, 0.01..=0.99))
                            .changed()
                        {
                            self.update_texture(ui.ctx()).unwrap();
                        }
                        ui.end_row();

                        ui.label(t!("results.gamma"));
                        if ui
                            .add(egui::Slider::new(&mut self.gamma, 0.0..=10.0))
                            .changed()
                        {
                            self.update_texture(ui.ctx()).unwrap();
                        }
                        ui.end_row();
                    });
            });
            self.histogram.ui(ui);
        });

        Ok(())
    }

    fn get_output_path(&self) -> PathBuf {
        if let Some(results) = &self.results {
            results.output_filename.clone()
        } else {
            dirs::home_dir().unwrap()
        }
    }
}

impl ResultViewPane {
    pub fn ui(&mut self, ui: &mut Ui) {
        self.options_ui(ui).unwrap();
        if let Some(handle) = &self.texture_handle {
            ui.add(egui::Image::from_texture(handle).shrink_to_fit())
                .context_menu(|ui| {
                    if ui.button(t!("results.save_as")).clicked() {
                        let output_path = self.get_output_path();
                        let filename = output_path.file_name().unwrap();

                        if let Some(path) = rfd::FileDialog::new()
                            .set_title(t!("results.save_as"))
                            .set_directory(output_path.parent().unwrap())
                            .set_file_name(filename.to_string_lossy())
                            .add_filter("TIFF", &["tif"])
                            .save_file()
                        {
                            println!("Saving To Path: {:?}", path);

                            if let Some(results) = &self.results {
                                let mut image_adjusted = results.image.clone();

                                image_adjusted.levels_with_gamma(
                                    0.0,
                                    1.0 - self.exposure as f32,
                                    self.gamma as f32,
                                );

                                image_adjusted
                                    .save(path.to_string_lossy().as_ref())
                                    .expect("Failed to save image");
                            } else {
                                panic!("Cannot save image. No image to save.");
                            }
                            ui.close_menu();
                        } else {
                            ui.close_menu();
                        }
                    }
                });
        }
    }
}
