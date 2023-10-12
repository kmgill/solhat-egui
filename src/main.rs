#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use anyhow::{anyhow, Result};
use eframe::egui;
use egui::ColorImage;
use egui::Pos2;
use egui::Vec2;
// use egui_extras::install_image_loaders;
use itertools::iproduct;
use serde::{Deserialize, Serialize};
use solhat::anaysis::frame_sigma_analysis;
use solhat::calibrationframe::CalibrationImage;
use solhat::calibrationframe::ComputeMethod;
use solhat::context::*;
use solhat::drizzle::Scale;
use solhat::limiting::frame_limit_determinate;
use solhat::offsetting::frame_offset_analysis;
use solhat::rotation::frame_rotation_analysis;
use solhat::ser::SerFile;
use solhat::ser::SerFrame;
use solhat::stacking::process_frame_stacking;
use solhat::target::Target;
use std::path::Path;
use std::path::PathBuf;

mod taskstatus;
use taskstatus::*;

mod cancel;
use cancel::*;

mod analysis;
use analysis::*;

mod process;
use process::*;

mod state;
use state::*;

#[macro_use]
extern crate stump;

#[macro_use]
extern crate lazy_static;

// https://github.com/emilk/egui/discussions/1574
pub(crate) fn load_icon() -> eframe::IconData {
    let (icon_rgba, icon_width, icon_height) = {
        let icon = include_bytes!("../assets/solhat_icon_32x32.png");
        let image = image::load_from_memory(icon)
            .expect("Failed to open icon path")
            .into_rgba8();
        let (width, height) = image.dimensions();
        let rgba = image.into_raw();
        (rgba, width, height)
    };

    eframe::IconData {
        rgba: icon_rgba,
        width: icon_width,
        height: icon_height,
    }
}

#[derive(Deserialize, Serialize)]
struct SolHat {
    state: state::ApplicationState,

    #[serde(skip_serializing, skip_deserializing)]
    thumbnail_light: Option<egui::TextureHandle>,

    #[serde(skip_serializing, skip_deserializing)]
    thumbnail_dark: Option<egui::TextureHandle>,

    #[serde(skip_serializing, skip_deserializing)]
    thumbnail_flat: Option<egui::TextureHandle>,

    #[serde(skip_serializing, skip_deserializing)]
    thumbnail_darkflat: Option<egui::TextureHandle>,

    #[serde(skip_serializing, skip_deserializing)]
    thumbnail_bias: Option<egui::TextureHandle>,
}

fn ser_frame_to_retained_image(ser_frame: &SerFrame) -> ColorImage {
    let mut copied = ser_frame.buffer.clone();
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
    // let f = egui::Image::new(ColorImage::from_rgb(size, &rgb));
    // egui::Image::from_color_image(
    //     "thumbnail_main",
    //     ColorImage::from_rgb(size, &rgb).try_into(),
    // )
}

#[tokio::main]
async fn main() -> Result<(), eframe::Error> {
    stump::set_min_log_level(stump::LogEntryLevel::DEBUG);
    info!("Starting SolHat-UI");

    let mut options = eframe::NativeOptions {
        icon_data: Some(load_icon()),
        initial_window_size: Some(Vec2 { x: 885.0, y: 650.0 }),
        min_window_size: Some(Vec2 { x: 885.0, y: 650.0 }),
        resizable: true,
        transparent: true,
        vsync: true,
        multisampling: 0,
        depth_buffer: 0,
        stencil_buffer: 0,
        ..Default::default()
    };

    // If the config file (literally a serialized version of the last run window state) errors on read
    // or doesn't exist, we'll just ignore it and start from scratch.
    let solhat = if let Ok(app_state) = ApplicationState::load_from_userhome() {
        options.initial_window_pos = Some(Pos2::new(
            app_state.window.window_pos_x as f32,
            app_state.window.window_pos_y as f32,
        ));
        // This don't work on Linux (Fedora KDE). Windows keep growing...Likely
        // related to egui's insistence on 1.5x UI scale?
        options.initial_window_size = Some(Vec2::new(
            app_state.window.window_width as f32,
            app_state.window.window_height as f32,
        ));
        println!("Creating application with previous settings");
        Box::new(SolHat {
            state: app_state,
            thumbnail_light: None,
            thumbnail_dark: None,
            thumbnail_flat: None,
            thumbnail_darkflat: None,
            thumbnail_bias: None,
        })
    } else {
        options.centered = true;
        println!("Loading application defaults");
        Box::<SolHat>::default()
    };

    eframe::run_native("SolHat", options, Box::new(|_cc| solhat))
}

impl Default for SolHat {
    fn default() -> Self {
        Self {
            state: ApplicationState::default(),
            thumbnail_light: None,
            thumbnail_dark: None,
            thumbnail_flat: None,
            thumbnail_darkflat: None,
            thumbnail_bias: None,
        }
    }
}

impl eframe::App for SolHat {
    fn on_close_event(&mut self) -> bool {
        self.state.save_to_userhome();
        true
    }

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.state.enforce_value_bounds();
        self.state.window.update_from_window_info(ctx, frame);

        self.on_update(ctx, frame);
    }
}

fn truncate_to(s: &str, max_len: usize) -> String {
    if s.len() < max_len {
        s.to_owned()
    } else {
        let t: String = "...".to_owned() + &s[(s.len() - max_len + 3)..];
        t
    }
}

macro_rules! create_file_input {
    ($ui:expr, $name:expr, $state:expr, $state_property:expr, $open_type_name:expr, $open_type_ext:expr) => {{
        $ui.label(&format!("{}:", $name));
        $ui.monospace(truncate_to(
            &$state_property.clone().unwrap_or("".to_owned()),
            35,
        ));
        if $ui.button("Open file…").clicked() {
            if let Some(path) = rfd::FileDialog::new()
                .set_title(&format!("Open {}", $name))
                .set_directory($state.window.get_last_opened_folder())
                .add_filter($open_type_name, &[$open_type_ext])
                .pick_file()
            {
                $state_property = Some(path.display().to_string());
                $state.window.update_last_opened_folder(&path);
            }
        }
        if $ui.button("Clear").clicked() {
            $state_property = None;
        }
        $ui.end_row();
    }};
}

macro_rules! show_ser_thumbnail {
    ($ui:expr, $path_option:expr, $texture_name:expr, $state_property:expr) => {{
        if let Some(ser_path) = &$path_option {
            if $state_property.is_none() {
                let texture = SolHat::load_ser_texture(&$ui, $texture_name, &ser_path);
                $state_property = Some(texture);
            }

            if let Some(texture) = &$state_property {
                $ui.add(egui::Image::from_texture(texture).shrink_to_fit());
            }
        } else {
            $state_property = None;
        }
    }};
}

impl SolHat {
    // #[allow(dead_code)]
    // fn load_thumbnail(&mut self, ui: &mut egui::Ui, force: bool) {
    //     if let Some(light_path) = &self.state.light {
    //         if self.state.thumbnail_main.is_none() || force {
    //             let ser_file = SerFile::load_ser(light_path).unwrap();
    //             let first_image: SerFrame = ser_file.get_frame(0).unwrap();
    //             let cimage = ser_frame_to_retained_image(&first_image);
    //             let texture: egui::TextureHandle =
    //                 ui.ctx().load_texture("foo", cimage, Default::default());
    //             ui.image(&texture);
    //             // self.state.thumbnail_main = Some();
    //         }
    //     } else {
    //         self.state.thumbnail_main = None;
    //     }
    // }

    fn load_ser_texture(
        ui: &egui::Ui,
        texture_name: &str,
        texture_path: &str,
    ) -> egui::TextureHandle {
        let ser_file = SerFile::load_ser(texture_path).unwrap();
        let first_image: SerFrame = ser_file.get_frame(0).unwrap();
        let cimage = ser_frame_to_retained_image(&first_image);
        ui.ctx()
            .load_texture(texture_name, cimage, Default::default())
    }

    fn on_update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // install_image_loaders(ctx);
        //ctx.set_pixels_per_point(1.0);
        // self.load_thumbnail(false);
        self.state.enforce_value_bounds();
        self.state.window.update_from_window_info(ctx, frame);

        egui::SidePanel::left("left_panel")
            .resizable(true)
            .show(ctx, |ui| {
                /////////////////////////////////
                // Left side controls:
                /////////////////////////////////

                ui.heading("Inputs");
                egui::Grid::new("process_grid_inputs")
                    .num_columns(2)
                    .spacing([40.0, 4.0])
                    .striped(true)
                    .show(ui, |ui| {
                        self.inputs_frame_contents(ui);
                    });
                ui.separator();

                ui.heading("Output");
                egui::Grid::new("process_grid_outputs")
                    .num_columns(2)
                    .spacing([40.0, 4.0])
                    .striped(true)
                    .show(ui, |ui| {
                        self.outputs_frame_contents(ui);
                    });
                ui.separator();

                ui.heading("Observation");
                egui::Grid::new("process_grid_observation")
                    .num_columns(2)
                    .spacing([40.0, 4.0])
                    .striped(true)
                    .show(ui, |ui| {
                        self.observation_frame_contents(ui);
                    });
                ui.separator();

                ui.heading("Process Options");
                egui::Grid::new("process_grid_options")
                    .num_columns(2)
                    .spacing([40.0, 4.0])
                    .striped(true)
                    .show(ui, |ui| {
                        self.options_frame_contents(ui);
                    });
                ui.separator();

                match get_task_status() {
                    Some(TaskStatus::TaskPercentage(task_name, len, cnt)) => {
                        ui.vertical_centered(|ui| {
                            ui.monospace(task_name);
                            let pct = if len > 0 {
                                cnt as f32 / len as f32
                            } else {
                                0.0
                            };
                            ui.add(egui::ProgressBar::new(pct).animate(true).show_percentage());
                            //ui.spinner();
                        });
                    }
                    None => {
                        ui.vertical_centered(|ui| {
                            ui.add_enabled_ui(self.enable_start(), |ui| {
                                if ui.button("START").clicked() {
                                    let output_filename =
                                        self.state.assemble_output_filename().unwrap();
                                    self.run(output_filename);
                                    // Do STUFF!
                                }
                            });
                        });
                    }
                }
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical(|ui| {
                ///////////////////////////////////////
                // Right side controls
                ///////////////////////////////////////

                ui.horizontal(|ui| {
                    ui.selectable_value(
                        &mut self.state.window.selected_preview_pane,
                        PreviewPane::Light,
                        "Light",
                    );
                    ui.selectable_value(
                        &mut self.state.window.selected_preview_pane,
                        PreviewPane::Dark,
                        "Dark",
                    );
                    ui.selectable_value(
                        &mut self.state.window.selected_preview_pane,
                        PreviewPane::Flat,
                        "Flat",
                    );
                    ui.selectable_value(
                        &mut self.state.window.selected_preview_pane,
                        PreviewPane::DarkFlat,
                        "Dark Flat",
                    );
                    ui.selectable_value(
                        &mut self.state.window.selected_preview_pane,
                        PreviewPane::Bias,
                        "Bias",
                    );
                    ui.selectable_value(
                        &mut self.state.window.selected_preview_pane,
                        PreviewPane::Analysis,
                        "Analysis",
                    );
                });
                ui.separator();

                match self.state.window.selected_preview_pane {
                    PreviewPane::Light => show_ser_thumbnail!(
                        ui,
                        self.state.light,
                        "thumbnail-light",
                        self.thumbnail_light
                    ),
                    PreviewPane::Dark => show_ser_thumbnail!(
                        ui,
                        self.state.dark,
                        "thumbnail-dark",
                        self.thumbnail_dark
                    ),
                    PreviewPane::Flat => show_ser_thumbnail!(
                        ui,
                        self.state.flat,
                        "thumbnail-flat",
                        self.thumbnail_flat
                    ),
                    PreviewPane::DarkFlat => show_ser_thumbnail!(
                        ui,
                        self.state.darkflat,
                        "thumbnail-darkflat",
                        self.thumbnail_darkflat
                    ),
                    PreviewPane::Bias => show_ser_thumbnail!(
                        ui,
                        self.state.bias,
                        "thumbnail-bias",
                        self.thumbnail_bias
                    ),
                    PreviewPane::Analysis => {}
                }
            });
        });
    }

    fn outputs_frame_contents(&mut self, ui: &mut egui::Ui) {
        // Light Frames
        ui.label("Output Folder:");
        ui.horizontal(|ui| {
            if let Some(output_dir) = &self.state.output_dir {
                ui.monospace(output_dir);
            }
            if ui.button("Open folder...").clicked() {
                if let Some(path) = rfd::FileDialog::new().pick_folder() {
                    self.state.output_dir = Some(path.display().to_string());
                }
            }
        });
        ui.end_row();

        if let Ok(output_filename) = self.state.assemble_output_filename() {
            ui.label("Output Filename:");
            ui.monospace(output_filename.to_string_lossy().as_ref());
        }
    }

    fn inputs_frame_contents(&mut self, ui: &mut egui::Ui) {
        egui::Grid::new("inputs_3x3_lights")
            .num_columns(4)
            .spacing([40.0, 4.0])
            .striped(true)
            .show(ui, |ui| {
                create_file_input!(ui, "Light", self.state, self.state.light, "SER", "ser");
                create_file_input!(ui, "Dark", self.state, self.state.dark, "SER", "ser");
                create_file_input!(ui, "Flat", self.state, self.state.flat, "SER", "ser");
                create_file_input!(
                    ui,
                    "Dark Flat",
                    self.state,
                    self.state.darkflat,
                    "SER",
                    "ser"
                );
                create_file_input!(ui, "Bias", self.state, self.state.bias, "SER", "ser");
                create_file_input!(
                    ui,
                    "Hot Pixal Map",
                    self.state,
                    self.state.hot_pixel_map,
                    "TOML",
                    "toml"
                );
            });
        ui.end_row();
    }

    fn observation_frame_contents(&mut self, ui: &mut egui::Ui) {
        ui.label("Observer Latitude:");
        ui.add(
            egui::DragValue::new(&mut self.state.obs_latitude)
                .min_decimals(1)
                .max_decimals(4)
                .speed(1.0),
        );
        ui.end_row();

        ui.label("Observer Longitude:");
        ui.add(
            egui::DragValue::new(&mut self.state.obs_longitude)
                .min_decimals(1)
                .max_decimals(4)
                .speed(1.0),
        );
        ui.end_row();

        ui.label("Target:");
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.state.target, Target::Sun, "Sun");
            ui.selectable_value(&mut self.state.target, Target::Moon, "Moon");
            ui.selectable_value(&mut self.state.target, Target::None, "None / Prealigned");
        });
        ui.end_row();
    }

    fn options_frame_contents(&mut self, ui: &mut egui::Ui) {
        ui.label("Object Detection Threshold:");
        ui.add(egui::DragValue::new(&mut self.state.obj_detection_threshold).speed(10.0));
        if ui.add(egui::Button::new("Test")).clicked() {
            // Do stuff
        }
        ui.end_row();

        ui.label("Analysis Window Size:");
        ui.add(egui::DragValue::new(&mut self.state.analysis_window_size).speed(1.0));
        if ui.add(egui::Button::new("Run Analysis")).clicked() {
            // Do stuff
            self.run_analysis();
        }
        ui.end_row();

        ui.label("Drizzle:");
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.state.drizzle_scale, Scale::Scale1_0, "None");
            ui.selectable_value(&mut self.state.drizzle_scale, Scale::Scale1_5, "1.5x");
            ui.selectable_value(&mut self.state.drizzle_scale, Scale::Scale2_0, "2.0x");
            ui.selectable_value(&mut self.state.drizzle_scale, Scale::Scale3_0, "3.0x");
        });
        ui.end_row();

        ui.label("Use Maximum Frames:");
        ui.add(egui::DragValue::new(&mut self.state.max_frames).speed(10.0));
        ui.end_row();

        ui.label("Minimum Sigma:");
        ui.add(egui::DragValue::new(&mut self.state.min_sigma).speed(1.0));
        ui.end_row();

        ui.label("Maximum Sigma:");
        ui.add(egui::DragValue::new(&mut self.state.max_sigma).speed(1.0));
        ui.end_row();

        ui.label("Include Top Percentage:");
        ui.add(egui::DragValue::new(&mut self.state.top_percentage).speed(1.0));
        ui.end_row();

        ui.label("");
        ui.add(egui::Checkbox::new(
            &mut self.state.decorrelated_colors,
            "Decorrelated Colors",
        ));
        ui.end_row();

        ui.label("");
        ui.add(egui::Checkbox::new(
            &mut self.state.ld_correction,
            "Limb Darkening Correction",
        ));
        ui.end_row();

        ui.label("Limb Darkening Coefficient:");
        ui.add(egui::DragValue::new(&mut self.state.ld_coefficient).speed(0.1));
        ui.end_row();

        ui.label("Solar Disk Radius (Pixels):");
        ui.add(egui::DragValue::new(&mut self.state.solar_radius_pixels).speed(1.0));
        ui.end_row();

        ui.label("Filename Free Text:");
        ui.add(
            egui::TextEdit::singleline(&mut self.state.freetext).hint_text("Write something here"),
        );
        ui.end_row();
    }

    fn enable_start(&self) -> bool {
        self.state.light.is_some() && self.state.output_dir.is_some()
    }

    fn run(&mut self, output_filename: PathBuf) {
        let state_copy = self.state.clone();

        tokio::spawn(async move {
            {
                process::run_async(output_filename, state_copy)
                    .await
                    .unwrap();
            }
        });
    }

    fn run_analysis(&mut self) {
        let state_copy = self.state.clone();

        tokio::spawn(async move {
            {
                sigma::run_sigma_analysis(state_copy).await.unwrap();
            }
        });
    }
}
