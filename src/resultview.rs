use crate::histogram::Histogram;
use crate::imageutil;
use crate::process::RunResultsContainer;
use crate::toggle::toggle;
use anyhow::{Error, Result};
use egui::Ui;
use sciimg::prelude::Image;
use sciimg::unsharp::RgbImageUnsharpMask;
use std::fmt;
use std::path::PathBuf;

#[derive(Clone, Eq, PartialEq)]
enum ZoomType {
    Fit,
    FullSize,
}

impl fmt::Display for ZoomType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ZoomType::Fit => f.write_str(&t!("results.shrink_to_fit")),
            ZoomType::FullSize => f.write_str(&t!("results.full_size")),
        }
        // write!(f, "{:?}", self)
    }
}

pub struct ResultViewPane {
    texture_handle: Option<egui::TextureHandle>,
    texture_name: String,
    results: Option<RunResultsContainer>,
    histogram: Histogram,
    exposure: f64,
    gamma: f64,
    unsharp_mask: bool,
    unsharp_sigma: f64,
    unsharp_amount: f64,
    zoom: ZoomType,
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
            unsharp_mask: false,
            unsharp_amount: 1.0,
            unsharp_sigma: 1.3,
            zoom: ZoomType::Fit,
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
            if results.image.is_some() {
                self.histogram.compute_from_image(&results.image.clone().unwrap());
            }
            Ok(())
        } else {
            Err(Error::msg("No ser file loaded"))
        }
    }

    fn apply_filters(&self, image: &Image) -> Image {
        let mut image_adjusted = image.clone();

        image_adjusted.levels_with_gamma(0.0, 1.0 - self.exposure as f32, 1.0 / self.gamma as f32);

        if self.unsharp_mask {
            image_adjusted.unsharp_mask(self.unsharp_sigma as f32, self.unsharp_amount as f32);
        }

        image_adjusted
    }

    fn update_texture(&mut self, ctx: &egui::Context) -> Result<()> {
        if let Some(results) = &self.results {

            if results.image.is_some() {
                let image_adjusted = self.apply_filters(&results.image.clone().unwrap());

                let cimage = imageutil::sciimg_to_color_image(&image_adjusted);
                self.texture_handle =
                    Some(ctx.load_texture(&self.texture_name, cimage, Default::default()));
            }
            Ok(())
        } else {
            Err(Error::msg("No ser file loaded"))
        }
    }

    pub fn set_image(&mut self, results: &RunResultsContainer, ctx: &egui::Context) -> Result<()> {
        self.results = Some(results.clone());
        self.update_texture(ctx)?;
        self.update_histogram()?;

        Ok(())
    }

    fn options_ui(&mut self, ui: &mut Ui) -> Result<()> {
        // if let Some(results) = &self.results {
        //     ui.horizontal(|ui| {
        //         ui.label(t!("results.output_filename"));

        //         ui.label(results.output_filename.to_string_lossy().as_ref());
        //     });
        // }

        // if let Some(results) = &self.results {
        //     ui.horizontal(|ui| {
        //         ui.label(t!("results.num_images_used"));
        //         ui.label(results.num_frames_used.to_string());
        //     });
        // }

        let refresh_icon = egui::include_image!("../assets/refresh.svg");

        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                egui::Grid::new("metadata")
                    .num_columns(3)
                    .spacing([40.0, 4.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label(t!("results.exposure"));
                        if ui
                            .add(egui::Slider::new(&mut self.exposure, 0.01..=0.99))
                            .changed()
                        {
                            self.update_texture(ui.ctx()).unwrap();
                        }
                        if ui
                            .add(egui::Button::image_and_text(
                                refresh_icon.clone(),
                                t!("results.reset"),
                            ))
                            .clicked()
                        {
                            self.exposure = 0.0;
                            self.update_texture(ui.ctx()).unwrap();
                        }

                        ui.end_row();

                        ui.label(t!("results.gamma"));
                        if ui
                            .add(egui::Slider::new(&mut self.gamma, 0.05..=10.0))
                            .changed()
                        {
                            self.update_texture(ui.ctx()).unwrap();
                        }
                        if ui
                            .add(egui::Button::image_and_text(
                                refresh_icon.clone(),
                                t!("results.reset"),
                            ))
                            .clicked()
                        {
                            self.gamma = 1.0;
                            self.update_texture(ui.ctx()).unwrap();
                        }
                        ui.end_row();

                        ui.label(t!("results.zoom"));

                        egui::ComboBox::from_label("")
                            .selected_text(format!("{}", self.zoom))
                            .show_ui(ui, |ui| {
                                ui.style_mut().wrap = Some(false);
                                ui.set_min_width(60.0);
                                ui.selectable_value(
                                    &mut self.zoom,
                                    ZoomType::Fit,
                                    t!("results.shrink_to_fit"),
                                );
                                ui.selectable_value(
                                    &mut self.zoom,
                                    ZoomType::FullSize,
                                    t!("results.full_size"),
                                );
                            });

                        ui.end_row();
                        ui.label(t!("results.unsharp_masking"));
                        if ui.add(toggle(&mut self.unsharp_mask)).changed() {
                            self.update_texture(ui.ctx()).unwrap();
                        }
                        ui.end_row();

                        ui.label(t!("results.sigma"));
                        if ui
                            .add(egui::Slider::new(&mut self.unsharp_sigma, 0.05..=10.0))
                            .changed()
                        {
                            self.update_texture(ui.ctx()).unwrap();
                        }
                        ui.end_row();

                        ui.label(t!("results.amount"));
                        if ui
                            .add(egui::Slider::new(&mut self.unsharp_amount, 0.0..=100.0))
                            .changed()
                        {
                            self.update_texture(ui.ctx()).unwrap();
                        }
                    });
            });
            self.histogram.ui(ui);
        });

        Ok(())
    }

    fn get_output_path(&self) -> PathBuf {
        if let Some(results) = &self.results {
            if results.output_filename.is_some() {
                results.output_filename.clone().unwrap()
            } else {
                dirs::home_dir().unwrap()
            }
        } else {
            dirs::home_dir().unwrap()
        }
    }
}

impl ResultViewPane {
    pub fn ui(&mut self, ui: &mut Ui) {
        self.options_ui(ui).unwrap();
        if let Some(handle) = &self.texture_handle {
            //egui::ScrollArea::both().show(ui, |ui| {

            egui::ScrollArea::both().show(ui, |ui| {
                let image = egui::Image::from_texture(handle);
                ui.add(match self.zoom {
                    ZoomType::Fit => image.shrink_to_fit(),
                    ZoomType::FullSize => image,
                })
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
                                if results.image.is_some() {
                                    let image_adjusted = self.apply_filters(&results.image.clone().unwrap());

                                    image_adjusted
                                        .save(path.to_string_lossy().as_ref())
                                        .expect("Failed to save image");
                                } else {
                                    panic!("Cannot save image: Process resulted in error")
                                }
                            } else {
                                panic!("Cannot save image. No image to save.");
                            }
                            ui.close_menu();
                        } else {
                            ui.close_menu();
                        }
                    }
                });
            });
        }
    }
}
