use anyhow::{anyhow, Result};
use eframe::egui;
use serde::{Deserialize, Serialize};
use solhat::context::*;
use solhat::drizzle::Scale;
use solhat::target::Target;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

#[derive(Default, Deserialize, Serialize, Eq, PartialEq, Clone)]
pub enum PreviewPane {
    #[default]
    Light,
    Dark,
    Flat,
    DarkFlat,
    Bias,
    Analysis,
    Results,
}

#[derive(Default, Deserialize, Serialize, Clone)]
pub struct WindowState {
    pub last_opened_folder: Option<PathBuf>,
    pub window_pos_x: usize,
    pub window_pos_y: usize,
    pub window_width: usize,
    pub window_height: usize,
    pub fullscreen: bool,
    pub theme: String,
    pub selected_preview_pane: PreviewPane,
}

impl WindowState {
    pub fn get_last_opened_folder(&self) -> PathBuf {
        if self.last_opened_folder.is_some() {
            self.last_opened_folder.to_owned().unwrap()
        } else {
            std::env::current_dir().unwrap()
        }
    }

    pub fn update_last_opened_folder(&mut self, path: &Path) {
        info!("Last opened path: {:?}", path);
        self.last_opened_folder = if path.is_file() {
            Some(path.parent().unwrap().to_path_buf())
        } else {
            Some(path.to_path_buf())
        };
    }

    pub fn update_from_window_info(&mut self, _ctx: &egui::Context, frame: &mut eframe::Frame) {
        if let Some(position) = frame.info().window_info.position {
            self.window_pos_x = position.x as usize;
            self.window_pos_y = position.y as usize;
        }

        let dimension = frame.info().window_info.size;
        self.window_width = dimension.x as usize;
        self.window_height = dimension.y as usize;

        self.fullscreen = frame.info().window_info.fullscreen;
    }
}

#[derive(Deserialize, Serialize, Clone)]
pub struct ApplicationState {
    pub light: Option<String>,
    pub dark: Option<String>,
    pub flat: Option<String>,
    pub darkflat: Option<String>,
    pub bias: Option<String>,
    pub hot_pixel_map: Option<String>,
    pub output_dir: Option<String>,
    pub freetext: String,
    pub obs_latitude: f64,
    pub obs_longitude: f64,
    pub target: Target,
    pub obj_detection_threshold: f64,
    pub drizzle_scale: Scale,
    pub max_frames: usize,
    pub min_sigma: f64,
    pub max_sigma: f64,
    pub top_percentage: f64,
    pub decorrelated_colors: bool,
    pub analysis_window_size: usize,
    pub ld_correction: bool,
    pub ld_coefficient: f64,
    pub solar_radius_pixels: usize,
    pub crop_width: usize,
    pub crop_height: usize,
    pub vert_offset: i32,
    pub horiz_offset: i32,
    pub window: WindowState,
}

impl Default for ApplicationState {
    fn default() -> Self {
        Self {
            light: None,
            dark: None,
            flat: None,
            darkflat: None,
            bias: None,
            output_dir: None,
            freetext: "v1".to_owned(),
            obs_latitude: 34.0,
            obs_longitude: -118.0,
            target: Target::Sun,
            drizzle_scale: Scale::Scale1_0,
            obj_detection_threshold: 20000.0,
            hot_pixel_map: None,
            max_frames: 5000,
            min_sigma: 0.0,
            max_sigma: 1000.0,
            top_percentage: 100.0,
            window: WindowState::default(),
            decorrelated_colors: false,
            analysis_window_size: 128,
            ld_correction: false,
            ld_coefficient: 0.56,
            solar_radius_pixels: 768,
            crop_height: 0,
            crop_width: 0,
            vert_offset: 0,
            horiz_offset: 0,
        }
    }
}

impl ApplicationState {
    pub fn to_parameters(&self) -> ProcessParameters {
        ProcessParameters {
            input_files: if let Some(light) = &self.light {
                vec![light.to_owned()]
            } else {
                vec![]
            },
            obj_detection_threshold: self.obj_detection_threshold,
            obs_latitude: self.obs_latitude,
            obs_longitude: self.obs_longitude,
            target: self.target,
            crop_width: if self.crop_width == 0 {
                None
            } else {
                Some(self.crop_width)
            },
            crop_height: if self.crop_height == 0 {
                None
            } else {
                Some(self.crop_height)
            },
            vert_offset: self.vert_offset,
            horiz_offset: self.horiz_offset,
            max_frames: Some(self.max_frames),
            min_sigma: Some(self.min_sigma),
            max_sigma: Some(self.max_sigma),
            top_percentage: Some(self.top_percentage),
            drizzle_scale: self.drizzle_scale,
            initial_rotation: 0.0,
            flat_inputs: self.flat.to_owned(),
            dark_inputs: self.dark.to_owned(),
            darkflat_inputs: self.darkflat.to_owned(),
            bias_inputs: self.bias.to_owned(),
            hot_pixel_map: self.hot_pixel_map.to_owned(),
            analysis_window_size: self.analysis_window_size,
        }
    }

    pub fn load_from_userhome() -> Result<Self> {
        let config_file_path = dirs::home_dir().unwrap().join(".solhat/window-config.toml");
        if config_file_path.exists() {
            info!(
                "Window state config file exists at path: {:?}",
                config_file_path
            );
            let t = std::fs::read_to_string(config_file_path)?;
            Ok(toml::from_str(&t)?)
        } else {
            warn!("Window state config file does not exist. Will be created on exit");
            Err(anyhow!("Config file does not exist"))
        }
    }

    pub fn save_to_userhome(&self) {
        let toml_str = toml::to_string(&self).unwrap();
        let solhat_config_dir = dirs::home_dir().unwrap().join(".solhat/");
        if !solhat_config_dir.exists() {
            fs::create_dir(&solhat_config_dir).expect("Failed to create config directory");
        }
        let config_file_path = solhat_config_dir.join("window-config.toml");
        let mut f = File::create(config_file_path).expect("Failed to create config file");
        f.write_all(toml_str.as_bytes())
            .expect("Failed to write to config file");
        debug!("{}", toml_str);
    }

    pub fn assemble_output_filename(&self) -> Result<PathBuf> {
        let output_dir = if let Some(output_dir) = &self.output_dir {
            output_dir
        } else {
            return Err(anyhow!("Output directory not set"));
        };

        let base_filename = if let Some(input_file) = &self.light {
            Path::new(Path::new(input_file).file_name().unwrap())
                .file_stem()
                .unwrap()
        } else {
            return Err(anyhow!("Input light file not provided"));
        };

        let freetext = if !self.freetext.is_empty() {
            format!("_{}", self.freetext)
        } else {
            "".to_owned()
        };

        let drizzle = match self.drizzle_scale {
            Scale::Scale1_0 => "".to_owned(),
            _ => format!(
                "_{}",
                self.drizzle_scale.to_string().replace([' ', '.'], "")
            ),
        };

        let output_filename = format!(
            "{}_{:?}{}{}.tif",
            base_filename.to_string_lossy().as_ref(),
            self.target,
            drizzle,
            freetext
        );
        let output_path: PathBuf = Path::new(output_dir).join(output_filename);
        Ok(output_path)
    }

    pub fn enforce_value_bounds(&mut self) {
        if self.obs_latitude > 90.0 {
            self.obs_latitude = 90.0; // Hello North Pole!
        } else if self.obs_latitude < -90.0 {
            self.obs_latitude = -90.0; // Hello South Pole!
        }

        // Longitude -180 through 180 where -180 is west.
        if self.obs_longitude > 180.0 {
            self.obs_longitude = 180.0;
        } else if self.obs_longitude < -180.0 {
            self.obs_longitude = -180.0;
        }

        if self.top_percentage > 100.0 {
            self.top_percentage = 100.0;
        } else if self.top_percentage < 1.0 {
            self.top_percentage = 1.0;
        }

        if self.obj_detection_threshold < 1.0 {
            self.obj_detection_threshold = 1.0;
        }

        if self.min_sigma > self.max_sigma {
            self.min_sigma = self.max_sigma; // Depends on which value is currently being modified
        }

        if self.max_sigma < self.min_sigma {
            self.max_sigma = self.min_sigma; // Depends on which value is currently being modified,
                                             // though max_sigma can drive down min_sigma to zero.
        }

        if self.min_sigma < 0.0 {
            self.min_sigma = 0.0;
        }

        if self.max_sigma < 0.0 {
            self.max_sigma = 0.0;
        }
    }
}
