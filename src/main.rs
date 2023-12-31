#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use analysis::sigma::AnalysisSeries;
use anyhow::Result;
use eframe::egui;
use egui::Pos2;
use egui::Vec2;
use egui_extras::install_image_loaders;
use serde::{Deserialize, Serialize};
use solhat::drizzle::Scale;
use solhat::target::Target;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

mod histogram;
mod imageutil;
mod preview;
mod resultview;

mod toggle;
use toggle::toggle;

mod taskstatus;
use taskstatus::*;

mod cancel;

mod analysis;
use analysis::*;

mod process;
use process::RunResultsContainer;

mod state;
use state::*;

#[macro_use]
extern crate stump;

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate rust_i18n;
i18n!("locales", fallback = "en");

struct AnalysisResultsContainer {
    series: Option<AnalysisSeries>,
}

struct ImageResultsContainer {
    results: Option<RunResultsContainer>,
}

lazy_static! {
    static ref ANALYSIS_RESULTS: Arc<Mutex<AnalysisResultsContainer>> =
        Arc::new(Mutex::new(AnalysisResultsContainer { series: None }));
    static ref IMAGE_RESULTS: Arc<Mutex<ImageResultsContainer>> =
        Arc::new(Mutex::new(ImageResultsContainer { results: None }));
}

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

#[derive(Deserialize, Serialize, Default)]
struct SolHat {
    state: state::ApplicationState,

    #[serde(skip_serializing, skip_deserializing)]
    preview_light: preview::SerPreviewPane,

    #[serde(skip_serializing, skip_deserializing)]
    preview_dark: preview::SerPreviewPane,

    #[serde(skip_serializing, skip_deserializing)]
    preview_flat: preview::SerPreviewPane,

    #[serde(skip_serializing, skip_deserializing)]
    preview_darkflat: preview::SerPreviewPane,

    #[serde(skip_serializing, skip_deserializing)]
    preview_bias: preview::SerPreviewPane,

    #[serde(skip_serializing, skip_deserializing)]
    analysis_chart: analysis::AnalysisChart,

    #[serde(skip_serializing, skip_deserializing)]
    result_view: resultview::ResultViewPane,
}

#[tokio::main]
async fn main() -> Result<(), eframe::Error> {
    stump::set_min_log_level(stump::LogEntryLevel::DEBUG);
    info!("Starting SolHat-UI");
    stump::set_print(move |s| {
        println!("{}", s);
    });

    let mut options = eframe::NativeOptions {
        icon_data: Some(load_icon()),
        initial_window_size: Some(Vec2 {
            x: 1740.0,
            y: 950.0,
        }),
        min_window_size: Some(Vec2 {
            x: 1470.0,
            y: 840.0,
        }),
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

        options.initial_window_size = Some(Vec2::new(
            app_state.window.window_width as f32,
            app_state.window.window_height as f32,
        ));
        println!("Creating application with previous settings");
        Box::new(SolHat {
            state: app_state,
            ..Default::default()
        })
    } else {
        options.centered = true;
        println!("Loading application defaults");
        Box::<SolHat>::default()
    };

    eframe::run_native(&t!("apptitle"), options, Box::new(|_cc| solhat))
}

impl eframe::App for SolHat {
    fn on_close_event(&mut self) -> bool {
        self.state.save_to_userhome();
        true
    }

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.state.enforce_value_bounds();
        self.state.window.update_from_window_info(ctx, frame);

        self.on_update(ctx, frame).expect("Failed to update UI");
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
    ($ui:expr, $name:expr, $state:expr, $state_property:expr, $preview_property:expr, $open_type_name:expr, $open_type_ext:expr) => {{
        $ui.label(&format!("{}:", $name));
        $ui.monospace(truncate_to(
            &$state_property.clone().unwrap_or("".to_owned()),
            35,
        ))
        .on_hover_text(&$state_property.clone().unwrap_or("".to_owned()));
        if $ui.button(t!("inputs.open_file")).clicked() {
            if let Some(path) = rfd::FileDialog::new()
                .set_title(&format!("Open {}", $name))
                .set_directory($state.window.get_last_opened_folder())
                .add_filter($open_type_name, &[$open_type_ext])
                .pick_file()
            {
                $state_property = Some(path.display().to_string());

                if $open_type_name == "SER" {
                    $preview_property
                        .load_ser($ui.ctx(), &path.display().to_string())
                        .expect("Failed to load ser file");

                    if let Ok(tex_size) = $preview_property.size() {
                        if $state.crop_width == 0 {
                            $state.crop_width = tex_size[0];
                        }
                        if $state.crop_height == 0 {
                            $state.crop_height = tex_size[1];
                        }
                    }
                }

                $state.window.update_last_opened_folder(&path);
            }
        }
        if $ui.button(t!("inputs.clear")).clicked() {
            $preview_property.unload_ser();
            $state_property = None;
        }
        $ui.end_row();
    }};
}

impl SolHat {
    fn ensure_texture_loaded(
        ctx: &egui::Context,
        preview_pane: &mut preview::SerPreviewPane,
        ser_path: &Option<String>,
    ) -> Result<()> {
        if preview_pane.is_empty() && ser_path.is_some() {
            if let Some(ser_path) = &ser_path {
                preview_pane.load_ser(ctx, ser_path)?;
            }
        }
        Ok(())
    }

    fn on_update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) -> Result<()> {
        install_image_loaders(ctx);
        //ctx.set_pixels_per_point(1.0);
        // self.load_thumbnail(false);

        if let Ok(mut results) = ANALYSIS_RESULTS.lock() {
            if results.series.is_some() {
                self.analysis_chart.data = results.series.clone().unwrap();
                results.series = None;
                self.state.window.selected_preview_pane = PreviewPane::Analysis;
            } else if self.analysis_chart.is_empty()
                && self.state.window.selected_preview_pane == PreviewPane::Analysis
            {
                self.state.window.selected_preview_pane = PreviewPane::Light;
            }
        }

        if let Ok(mut img_results) = IMAGE_RESULTS.lock() {
            if let Some(results) = &img_results.results {
                self.result_view.set_image(results, ctx)?;
                self.state.window.selected_preview_pane = PreviewPane::Results;
                img_results.results = None;
            } else if self.result_view.is_empty()
                && self.state.window.selected_preview_pane == PreviewPane::Results
            {
                self.state.window.selected_preview_pane = PreviewPane::Light;
            }
        }

        SolHat::ensure_texture_loaded(ctx, &mut self.preview_light, &self.state.light)?;
        SolHat::ensure_texture_loaded(ctx, &mut self.preview_dark, &self.state.dark)?;
        SolHat::ensure_texture_loaded(ctx, &mut self.preview_flat, &self.state.flat)?;
        SolHat::ensure_texture_loaded(ctx, &mut self.preview_darkflat, &self.state.darkflat)?;
        SolHat::ensure_texture_loaded(ctx, &mut self.preview_bias, &self.state.bias)?;

        self.state.enforce_value_bounds();
        self.state.window.update_from_window_info(ctx, frame);

        let task_running = taskstatus::is_task_running();

        egui::SidePanel::left("left_panel")
            .resizable(true)
            .show(ctx, |ui| {
                /////////////////////////////////
                // Left side controls:
                /////////////////////////////////

                ui.add_enabled_ui(!task_running, |ui| {
                    self.inputs_frame_contents(ui, ctx);
                    ui.separator();

                    self.outputs_frame_contents(ui, ctx);
                    ui.separator();

                    self.observation_frame_contents(ui, ctx);
                    ui.separator();

                    self.options_frame_contents(ui, ctx);
                    ui.separator();
                });

                match get_task_status() {
                    Some(TaskStatus::TaskPercentage(task_name, len, cnt)) => {
                        ui.vertical_centered(|ui| {
                            ui.spacing_mut().button_padding = Vec2::new(18.0, 14.0);
                            let cancel_icon = egui::include_image!("../assets/cancel.svg");
                            if ui
                                .add(egui::Button::image_and_text(cancel_icon, t!("cancel")))
                                .clicked()
                            {
                                cancel::set_request_cancel();
                                ctx.request_repaint();
                            }

                            ui.horizontal(|ui| {
                                ui.monospace(task_name);
                                ui.spinner();
                            });

                            let pct = if len > 0 {
                                cnt as f32 / len as f32
                            } else {
                                0.0
                            };
                            ui.add(egui::ProgressBar::new(pct).show_percentage());
                        });
                    }
                    None => {
                        ui.vertical_centered(|ui| {
                            ui.add_enabled_ui(self.enable_start(), |ui| {
                                let start_icon = egui::include_image!("../assets/solve.svg");
                                ui.spacing_mut().button_padding = Vec2::new(18.0, 14.0);
                                if ui
                                    .add(egui::Button::image_and_text(start_icon, t!("start")))
                                    .clicked()
                                {
                                    let output_filename =
                                        self.state.assemble_output_filename().unwrap();
                                    self.run(output_filename);
                                    ctx.request_repaint();
                                }
                            });
                        });
                    }
                }

                ui.separator();

                ui.horizontal_wrapped(|ui| {
                    egui::widgets::global_dark_light_mode_switch(ui);
                    ui.separator();
                    ui.hyperlink("https://github.com/kmgill/solhat");
                });
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
                        t!("light"),
                    );
                    if self.state.dark.is_some() {
                        ui.selectable_value(
                            &mut self.state.window.selected_preview_pane,
                            PreviewPane::Dark,
                            t!("dark"),
                        );
                    }
                    if self.state.flat.is_some() {
                        ui.selectable_value(
                            &mut self.state.window.selected_preview_pane,
                            PreviewPane::Flat,
                            t!("flat"),
                        );
                    }
                    if self.state.darkflat.is_some() {
                        ui.selectable_value(
                            &mut self.state.window.selected_preview_pane,
                            PreviewPane::DarkFlat,
                            t!("darkflat"),
                        );
                    }
                    if self.state.bias.is_some() {
                        ui.selectable_value(
                            &mut self.state.window.selected_preview_pane,
                            PreviewPane::Bias,
                            t!("bias"),
                        );
                    }
                    if !self.analysis_chart.is_empty() {
                        ui.selectable_value(
                            &mut self.state.window.selected_preview_pane,
                            PreviewPane::Analysis,
                            t!("analysis"),
                        );
                    }
                    if !self.result_view.is_empty() {
                        ui.selectable_value(
                            &mut self.state.window.selected_preview_pane,
                            PreviewPane::Results,
                            t!("result"),
                        );
                    }
                });
                ui.separator();

                match self.state.window.selected_preview_pane {
                    PreviewPane::Light => self.preview_light.ui(ui),
                    PreviewPane::Dark => self.preview_dark.ui(ui),
                    PreviewPane::Flat => self.preview_flat.ui(ui),
                    PreviewPane::DarkFlat => self.preview_darkflat.ui(ui),
                    PreviewPane::Bias => self.preview_bias.ui(ui),
                    PreviewPane::Analysis => {
                        self.analysis_chart.ui(ui);
                    }
                    PreviewPane::Results => {
                        self.result_view.ui(ui);
                    }
                }
            });
        });

        Ok(())
    }

    fn outputs_frame_contents(&mut self, ui: &mut egui::Ui, _ctx: &egui::Context) {
        ui.heading(t!("output.title"));
        egui::Grid::new("process_grid_outputs")
            .num_columns(2)
            .spacing([40.0, 4.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label(t!("output.output_folder"));
                ui.horizontal(|ui| {
                    if let Some(output_dir) = &self.state.output_dir {
                        ui.monospace(output_dir);
                    }
                    if ui.button(t!("output.open_folder")).clicked() {
                        if let Some(path) = rfd::FileDialog::new().pick_folder() {
                            self.state.output_dir = Some(path.display().to_string());
                        }
                    }
                });
                ui.end_row();

                if let Ok(output_filename) = self.state.assemble_output_filename() {
                    ui.label(t!("output.output_filename"));
                    ui.monospace(truncate_to(output_filename.to_string_lossy().as_ref(), 55))
                        .on_hover_text(output_filename.to_string_lossy().as_ref());
                }
            });
    }

    fn inputs_frame_contents(&mut self, ui: &mut egui::Ui, _ctx: &egui::Context) {
        ui.heading(t!("inputs.title"));
        egui::Grid::new("inputs_3x3_lights")
            .num_columns(4)
            .spacing([40.0, 4.0])
            .striped(true)
            .show(ui, |ui| {
                create_file_input!(
                    ui,
                    t!("light"),
                    self.state,
                    self.state.light,
                    self.preview_light,
                    "SER",
                    "ser"
                );
                create_file_input!(
                    ui,
                    t!("dark"),
                    self.state,
                    self.state.dark,
                    self.preview_dark,
                    "SER",
                    "ser"
                );
                create_file_input!(
                    ui,
                    t!("flat"),
                    self.state,
                    self.state.flat,
                    self.preview_flat,
                    "SER",
                    "ser"
                );
                create_file_input!(
                    ui,
                    t!("darkflat"),
                    self.state,
                    self.state.darkflat,
                    self.preview_darkflat,
                    "SER",
                    "ser"
                );
                create_file_input!(
                    ui,
                    t!("bias"),
                    self.state,
                    self.state.bias,
                    self.preview_bias,
                    "SER",
                    "ser"
                );
                create_file_input!(
                    ui,
                    t!("hotpixelmap"),
                    self.state,
                    self.state.hot_pixel_map,
                    self.preview_light,
                    "TOML",
                    "toml"
                );
            });
        ui.end_row();
    }

    fn observation_frame_contents(&mut self, ui: &mut egui::Ui, _ctx: &egui::Context) {
        ui.heading(t!("observation.title"));

        egui::Grid::new("process_grid_observation")
            .num_columns(2)
            .spacing([40.0, 4.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label(t!("observation.title"));
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.state.target, Target::Sun, t!("observation.sun"));
                    ui.selectable_value(
                        &mut self.state.target,
                        Target::Moon,
                        t!("observation.moon"),
                    );
                    ui.selectable_value(
                        &mut self.state.target,
                        Target::None,
                        t!("observation.none"),
                    );
                });
            });

        ui.add_enabled_ui(self.state.target != Target::None, |ui| {
            egui::Grid::new("process_grid_observation_latlon")
                .num_columns(2)
                .spacing([40.0, 4.0])
                .striped(true)
                .show(ui, |ui| {
                    ui.label(t!("observation.obs_latitude"));
                    ui.add(
                        egui::DragValue::new(&mut self.state.obs_latitude)
                            .min_decimals(1)
                            .max_decimals(4)
                            .speed(1.0),
                    );
                    ui.end_row();

                    ui.label(t!("observation.obs_longitude"));
                    ui.add(
                        egui::DragValue::new(&mut self.state.obs_longitude)
                            .min_decimals(1)
                            .max_decimals(4)
                            .speed(1.0),
                    );
                    ui.end_row();
                });
        });
    }

    fn options_frame_contents(&mut self, ui: &mut egui::Ui, _ctx: &egui::Context) {
        ui.heading(t!("processoptions.title"));
        egui::Grid::new("process_grid_options")
            .num_columns(3)
            .spacing([40.0, 4.0])
            .striped(true)
            .show(ui, |ui| {
                let threshtest_icon = egui::include_image!("../assets/ellipse.svg");
                ui.label(t!("processoptions.obj_detect_thresh"));
                ui.add(egui::DragValue::new(&mut self.state.obj_detection_threshold).speed(10.0));

                ui.add_enabled_ui(!self.preview_light.is_empty(), |ui| {
                    if ui
                        .add(egui::Button::image_and_text(
                            threshtest_icon,
                            t!("processoptions.obj_detect_test"),
                        ))
                        .clicked()
                    {
                        self.preview_light
                            .threshold_test(ui, &self.state)
                            .expect("Failed threshold test");
                        self.state.window.selected_preview_pane = PreviewPane::Light;
                        // Do stuff
                    }
                });
                ui.end_row();

                let analysis_icon = egui::include_image!("../assets/chart.svg");
                ui.label(t!("processoptions.analysis_window_size"));
                ui.add(egui::DragValue::new(&mut self.state.analysis_window_size).speed(1.0));
                ui.add_enabled_ui(!self.preview_light.is_empty(), |ui| {
                    if ui
                        .add(egui::Button::image_and_text(
                            analysis_icon,
                            t!("processoptions.analysis_run"),
                        ))
                        .clicked()
                    {
                        // Do stuff
                        self.run_analysis();
                    }
                });
                ui.end_row();

                ui.label(t!("processoptions.drizzle"));
                ui.horizontal(|ui| {
                    ui.selectable_value(
                        &mut self.state.drizzle_scale,
                        Scale::Scale1_0,
                        t!("processoptions.drizzle_none"),
                    );
                    ui.selectable_value(
                        &mut self.state.drizzle_scale,
                        Scale::Scale1_5,
                        t!("processoptions.drizzle_15x"),
                    );
                    ui.selectable_value(
                        &mut self.state.drizzle_scale,
                        Scale::Scale2_0,
                        t!("processoptions.drizzle_20x"),
                    );
                    ui.selectable_value(
                        &mut self.state.drizzle_scale,
                        Scale::Scale3_0,
                        t!("processoptions.drizzle_30x"),
                    );
                });
                ui.end_row();

                ui.label(t!("processoptions.use_max_frames"));
                ui.add(egui::DragValue::new(&mut self.state.max_frames).speed(10.0));
                ui.end_row();

                ui.label(t!("processoptions.minimum_sigma"));
                ui.add(egui::DragValue::new(&mut self.state.min_sigma).speed(1.0));
                ui.end_row();

                ui.label(t!("processoptions.maximum_sigma"));
                ui.add(egui::DragValue::new(&mut self.state.max_sigma).speed(1.0));
                ui.end_row();

                ui.label(t!("processoptions.include_top_percent"));
                ui.add(egui::DragValue::new(&mut self.state.top_percentage).speed(1.0));
                ui.end_row();

                ui.label(t!("processoptions.decorrelated_colors"));
                ui.add(toggle(&mut self.state.decorrelated_colors));
                ui.end_row();

                ui.label(t!("processoptions.limb_dark_correction"));
                ui.add(toggle(&mut self.state.ld_correction));
                ui.end_row();

                ui.add_enabled_ui(self.state.ld_correction, |ui| {
                    ui.label(t!("processoptions.ldc_coefficient"));
                });

                ui.add_enabled_ui(self.state.ld_correction, |ui| {
                    ui.add(egui::DragValue::new(&mut self.state.ld_coefficient).speed(0.1));
                });
                ui.end_row();

                ui.add_enabled_ui(self.state.ld_correction, |ui| {
                    ui.label(t!("processoptions.ldc_solar_radius"));
                });
                ui.add_enabled_ui(self.state.ld_correction, |ui| {
                    ui.add(egui::DragValue::new(&mut self.state.solar_radius_pixels).speed(1.0));
                });
                ui.end_row();

                let refresh_icon = egui::include_image!("../assets/refresh.svg");

                ui.label(t!("processoptions.crop_width"));
                ui.add(egui::DragValue::new(&mut self.state.crop_width).speed(1.0));
                if !self.preview_light.is_empty() {
                    if ui
                        .add(egui::Button::image_and_text(
                            refresh_icon.clone(),
                            t!("processoptions.reset"),
                        ))
                        .clicked()
                    {
                        if let Ok(size) = self.preview_light.size() {
                            self.state.crop_width = size[0];
                        }
                    }
                }
                ui.end_row();

                ui.label(t!("processoptions.crop_height"));
                ui.add(egui::DragValue::new(&mut self.state.crop_height).speed(1.0));
                if self.state.light.is_some() {
                    if ui
                        .add(egui::Button::image_and_text(
                            refresh_icon,
                            t!("processoptions.reset"),
                        ))
                        .clicked()
                    {
                        if let Ok(size) = self.preview_light.size() {
                            self.state.crop_height = size[1];
                        }
                    }
                }
                ui.end_row();

                ui.label(t!("processoptions.horiz_offset"));
                ui.add(egui::DragValue::new(&mut self.state.horiz_offset).speed(1.0));
                ui.end_row();

                ui.label(t!("processoptions.vert_offset"));
                ui.add(egui::DragValue::new(&mut self.state.vert_offset).speed(1.0));
                ui.end_row();

                ui.label(t!("processoptions.filename_free_text"));
                ui.add(
                    egui::TextEdit::singleline(&mut self.state.freetext)
                        .hint_text(t!("processoptions.filename_hint")),
                );
                ui.end_row();
            });
    }

    fn enable_start(&self) -> bool {
        self.state.light.is_some() && self.state.output_dir.is_some()
    }

    fn run(&mut self, output_filename: PathBuf) {
        let state_copy = self.state.clone();
        set_task_status(&t!("tasks.starting"), 1, 1);

        tokio::spawn(async move {
            {
                let results = process::run_async(output_filename, state_copy)
                    .await
                    .unwrap();

                IMAGE_RESULTS.lock().unwrap().results = Some(results);
                set_task_completed();
            }
        });
    }

    fn run_analysis(&mut self) {
        let state_copy = self.state.clone();
        set_task_status(&t!("tasks.starting"), 1, 1);

        tokio::spawn(async move {
            {
                let analysis_data = sigma::run_sigma_analysis(state_copy).await.unwrap();
                // TODO: Seriously, Kevin, learn to do proper data flow. Come on.
                ANALYSIS_RESULTS.lock().unwrap().series = Some(analysis_data);
                set_task_completed();
            }
        });
    }
}
