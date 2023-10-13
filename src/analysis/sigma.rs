use anyhow::Result;
use charts::{Chart, Color, LineSeriesView, MarkerType, ScaleLinear};
use rayon::prelude::*;
use sciimg::{max, min, quality};
use solhat::calibrationframe::CalibrationImage;
use solhat::context::ProcessContext;
use solhat::framerecord::FrameRecord;
use std::sync::{Arc, Mutex};

use crate::cancel::{self, *};
use crate::state::ApplicationState;
use crate::taskstatus::*;

///////////////////////////////////////////////////////
// Sigma Anaysis
///////////////////////////////////////////////////////

lazy_static! {
    // NOTE: Concurrent processing threads will stomp on each other, but at least
    // they'll do it in proper turn.  Also, this is stupid and can't stay this way.
    static ref COUNTER: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));
}

#[derive(Debug)]
pub struct AnalysisRange {
    min: f64,
    max: f64,
}

#[derive(Debug, Clone)]
pub struct AnalysisSeries {
    sigma_list: Vec<f64>,
}

impl AnalysisSeries {
    pub fn sorted_list(&self) -> Vec<f64> {
        let mut sorted = self.sigma_list.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        sorted.reverse();
        sorted
    }

    pub fn minmax(&self) -> AnalysisRange {
        let mut mn = std::f64::MAX;
        let mut mx = std::f64::MIN;

        self.sigma_list.iter().for_each(|s| {
            mn = min!(*s, mn);
            mx = max!(*s, mx);
        });

        AnalysisRange { min: mn, max: mx }
    }

    pub fn sma(&self, window: usize) -> Vec<f64> {
        let half_win = window / 2;
        let mut sma: Vec<f64> = vec![];
        (0..self.sigma_list.len()).for_each(|i| {
            let start = if i <= half_win { 0 } else { i - half_win };

            let end = if i + half_win <= self.sigma_list.len() {
                i + half_win
            } else {
                self.sigma_list.len()
            };
            let s = self.sigma_list[start..end].iter().sum::<f64>() / (end - start) as f64;
            sma.push(s);
        });
        sma
    }
}

pub async fn run_sigma_analysis(
    app_state: ApplicationState,
) -> Result<AnalysisSeries, TaskCompletion> {
    let params = app_state.to_parameters();
    let context = match ProcessContext::create_with_calibration_frames(
        &params,
        CalibrationImage::new_empty(),
        CalibrationImage::new_empty(),
        CalibrationImage::new_empty(),
        CalibrationImage::new_empty(),
    ) {
        Ok(context) => context,
        Err(why) => return Err(cancel::TaskCompletion::Error(format!("Error: {:?}", why))),
    };

    check_cancel_status()?;
    let frame_count = context.frame_records.len();
    *COUNTER.lock().unwrap() = 0;
    set_task_status("Frame Analysis", frame_count, 0);
    let frame_records = match frame_analysis_window_size(
        &context,
        context.parameters.analysis_window_size,
        move |fr| {
            info!(
                "frame_sigma_analysis(): Frame processed with sigma {}",
                fr.sigma
            );

            let mut c = COUNTER.lock().unwrap();
            *c += 1;
            set_task_status("Frame Analysis", frame_count, *c);
            // check_cancel_status(&sender)
        },
    ) {
        Ok(frame_records) => frame_records,
        Err(why) => return Err(cancel::TaskCompletion::Error(format!("Error: {:?}", why))),
    };

    let mut sigma_list: Vec<f64> = vec![];
    frame_records
        .iter()
        .filter(|fr| {
            let min_sigma = context.parameters.min_sigma.unwrap_or(std::f64::MIN);
            let max_sigma = context.parameters.max_sigma.unwrap_or(std::f64::MAX);
            fr.sigma >= min_sigma && fr.sigma <= max_sigma
        })
        .for_each(|fr| {
            sigma_list.push(fr.sigma);
        });

    set_task_completed();

    Ok(AnalysisSeries { sigma_list })
}

/// Combined method of center-of-mass and sigma analysis. This is to limit the number of
/// frame reads from disk which are rather expensive in terms of CPU and time.
pub fn frame_analysis_window_size<F>(
    context: &ProcessContext,
    window_size: usize,
    on_frame_checked: F,
) -> Result<Vec<FrameRecord>>
where
    F: Fn(&FrameRecord) + Send + Sync + 'static,
{
    let frame_records: Vec<FrameRecord> = context
        .frame_records
        .par_iter()
        .map(|fr| {
            let mut fr_copy = fr.clone();
            let frame = fr.get_frame(context).expect("");

            fr_copy.offset = frame
                .buffer
                .calc_center_of_mass_offset(context.parameters.obj_detection_threshold as f32, 0);

            let x = frame.buffer.width / 2 + fr_copy.offset.h as usize;
            let y = frame.buffer.height / 2 + fr_copy.offset.v as usize;

            // If monochrome, this will perform the analysis on the only band. If RGB, we perform analysis
            // on the red band.
            fr_copy.sigma = quality::get_point_quality_estimation_on_buffer(
                frame.buffer.get_band(0),
                window_size,
                x,
                y,
            ) as f64;

            on_frame_checked(&fr_copy);
            fr_copy
        })
        .collect();
    Ok(frame_records)
}

// Based on https://github.com/askanium/rustplotlib/blob/master/examples/line_series_chart.rs
pub fn create_chart(data: &AnalysisSeries, width: isize, height: isize) -> Result<String> {
    let (top, right, bottom, left) = (0, 40, 50, 60);

    let x = ScaleLinear::new()
        .set_domain(vec![0_f32, data.sigma_list.len() as f32])
        .set_range(vec![0, width - left - right]);

    let rng = data.minmax();

    let y = ScaleLinear::new()
        .set_domain(vec![rng.min as f32, rng.max as f32])
        .set_range(vec![height - top - bottom, 0]);

    let line_data_1: Vec<(f32, f32)> = data
        .sorted_list()
        .iter()
        .enumerate()
        .map(|(i, s)| (i as f32, *s as f32))
        .collect();

    let line_data_2: Vec<(f32, f32)> = data
        .sma(data.sigma_list.len() / 20)
        .iter()
        .enumerate()
        .map(|(i, s)| (i as f32, *s as f32))
        .collect();

    let line_data_3: Vec<(f32, f32)> = data
        .sigma_list
        .iter()
        .enumerate()
        .map(|(i, s)| (i as f32, *s as f32))
        .collect();

    let line_view_1 = LineSeriesView::new()
        .set_x_scale(&x)
        .set_y_scale(&y)
        .set_marker_type(MarkerType::X)
        .set_label_visibility(false)
        .set_marker_visibility(false)
        .set_colors(Color::from_vec_of_hex_strings(vec!["#AAAAAA"]))
        .load_data(&line_data_1)
        .unwrap();

    let line_view_2 = LineSeriesView::new()
        .set_x_scale(&x)
        .set_y_scale(&y)
        .set_marker_type(MarkerType::X)
        .set_label_visibility(false)
        .set_marker_visibility(false)
        .set_colors(Color::from_vec_of_hex_strings(vec!["#FF4700"]))
        .load_data(&line_data_2)
        .unwrap();

    let line_view_3 = LineSeriesView::new()
        .set_x_scale(&x)
        .set_y_scale(&y)
        .set_marker_type(MarkerType::X)
        .set_label_visibility(false)
        .set_marker_visibility(false)
        .set_colors(Color::from_vec_of_hex_strings(vec!["#333333"]))
        .load_data(&line_data_3)
        .unwrap();

    // Generate and save the chart.
    let svg = Chart::new()
        .set_width(width)
        .set_height(height)
        .set_margins(top, right, bottom, left)
        .add_view(&line_view_3)
        .add_view(&line_view_2)
        .add_view(&line_view_1)
        .add_axis_bottom(&x)
        .add_axis_left(&y)
        .add_left_axis_label("Sigma Quality")
        .add_bottom_axis_label("Frame #")
        .to_string()
        .unwrap();
    Ok(svg)
}
