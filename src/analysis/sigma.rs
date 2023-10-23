use anyhow::Result;
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

#[derive(Debug, Clone, Default)]
pub struct AnalysisSeries {
    pub sigma_list: Vec<f64>,
}

#[allow(dead_code)]
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
    set_task_status(&t!("tasks.frame_analysis"), frame_count, 0);
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
            set_task_status(&t!("tasks.frame_analysis"), frame_count, *c);
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
