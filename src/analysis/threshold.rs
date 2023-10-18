use crate::taskstatus::*;
use anyhow::Result;
use sciimg::prelude::*;
use solhat::calibrationframe::CalibrationImage;
use solhat::context::{ProcessContext, ProcessParameters};
use solhat::threshtest::compute_rgb_threshtest_image;

///////////////////////////////////////////////////////
/// Threshold Testing
///////////////////////////////////////////////////////

pub fn run_thresh_test(params: &ProcessParameters) -> Result<Image> {
    set_task_status("Processing Threshold Test", 2, 1);
    let context = ProcessContext::create_with_calibration_frames(
        params,
        CalibrationImage::new_empty(),
        CalibrationImage::new_empty(),
        CalibrationImage::new_empty(),
        CalibrationImage::new_empty(),
    )?;

    let first_frame = context.frame_records[0].get_frame(&context)?;
    let result = compute_rgb_threshtest_image(
        &first_frame.buffer,
        context.parameters.obj_detection_threshold as f32,
    );

    set_task_completed();
    Ok(result)
}
