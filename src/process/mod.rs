use anyhow::Result;
use sciimg::prelude::Image;
use solhat::calibrationframe::{CalibrationImage, ComputeMethod};
// use solhat::anaysis::frame_sigma_analysis_window_size;
use solhat::context::{ProcessContext, ProcessParameters};
use solhat::drizzle::BilinearDrizzle;
use solhat::framerecord::FrameRecord;
use solhat::ldcorrect;
use solhat::limiting::frame_limit_determinate;
// use solhat::offsetting::frame_offset_analysis;
use solhat::rotation::frame_rotation_analysis;
use solhat::stacking::process_frame_stacking;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::analysis::sigma::frame_analysis_window_size;
use crate::cancel::*;
use crate::state::*;
use crate::taskstatus::*;

#[derive(Clone)]
pub struct RunResultsContainer {
    pub image: Image,
    pub context: ProcessParameters,
    pub output_filename: PathBuf,
    pub num_frames_used: usize,
}

pub async fn run_async(
    output_filename: PathBuf,
    app_state: ApplicationState,
) -> Result<RunResultsContainer> {
    info!("Async task started");

    let mut context = build_solhat_context(&app_state)?;

    /////////////////////////////////////////////////////////////
    /////////////////////////////////////////////////////////////

    context.frame_records = frame_sigma_analysis(&context)?;

    /////////////////////////////////////////////////////////////
    /////////////////////////////////////////////////////////////

    context.frame_records = frame_limiting(&context)?;

    /////////////////////////////////////////////////////////////
    /////////////////////////////////////////////////////////////

    context.frame_records = frame_rotation(&context)?;

    /////////////////////////////////////////////////////////////
    /////////////////////////////////////////////////////////////

    if context.frame_records.is_empty() {
        Err(anyhow!("Zero frames to stack. Cannot continue"))
    } else {
        let drizzle_output = drizzle_stacking(&context)?;

        check_cancel_status()?;
        set_task_status(&t!("tasks.merging_stack_buffers"), 0, 0);
        let stacked_buffer = drizzle_output.get_finalized().unwrap();

        let do_ld_correction = app_state.ld_correction;
        let solar_radius = app_state.solar_radius_pixels;
        let ld_coefficient = app_state.ld_coefficient;
        let mut corrected_buffer = if do_ld_correction {
            set_task_status(&t!("tasks.apply_limb_correction"), 0, 0);
            ldcorrect::limb_darkening_correction_on_image(
                &stacked_buffer,
                solar_radius,
                &vec![ld_coefficient],
                10.0,
                false,
            )?
        } else {
            stacked_buffer
        };

        // Let the user know some stuff...
        let (stackmin, stackmax) = corrected_buffer.get_min_max_all_channel();
        info!(
            "    Stack Min/Max : {}, {} ({} images)",
            stackmin,
            stackmax,
            context.frame_records.len()
        );

        set_task_status(&t!("tasks.normalizing_data"), 0, 0);
        if app_state.decorrelated_colors {
            corrected_buffer.normalize_to_16bit_decorrelated();
        } else {
            corrected_buffer.normalize_to_16bit();
        }

        set_task_status(&t!("tasks.saving_to_disk"), 0, 0);
        info!(
            "Final image size: {}, {}",
            corrected_buffer.width, corrected_buffer.height
        );

        // Save finalized image to disk
        set_task_status(&t!("tasks.saving"), 0, 0);
        corrected_buffer.save(output_filename.to_string_lossy().as_ref())?;

        // The user will likely never see this actually appear on screen
        set_task_status(&t!("tasks.done"), 1, 1);

        Ok(RunResultsContainer {
            image: corrected_buffer,
            context: context.parameters,
            output_filename: output_filename.to_owned(),
            num_frames_used: context.frame_records.len(),
        })
    }
}

fn build_solhat_context(app_state: &ApplicationState) -> Result<ProcessContext> {
    let params = app_state.to_parameters();

    set_task_status(&t!("tasks.processing_master_flat"), 0, 0);
    let master_flat = if let Some(inputs) = &params.flat_inputs {
        info!("Processing master flat...");
        CalibrationImage::new_from_file(inputs, ComputeMethod::Mean)?
    } else {
        CalibrationImage::new_empty()
    };

    check_cancel_status()?;

    set_task_status(&t!("tasks.processing_master_dark_flat"), 0, 0);
    let master_darkflat = if let Some(inputs) = &params.darkflat_inputs {
        info!("Processing master dark flat...");
        CalibrationImage::new_from_file(inputs, ComputeMethod::Mean)?
    } else {
        CalibrationImage::new_empty()
    };

    check_cancel_status()?;

    set_task_status(&t!("tasks.processing_master_dark"), 0, 0);
    let master_dark = if let Some(inputs) = &params.dark_inputs {
        info!("Processing master dark...");
        CalibrationImage::new_from_file(inputs, ComputeMethod::Mean)?
    } else {
        CalibrationImage::new_empty()
    };

    check_cancel_status()?;

    set_task_status(&t!("tasks.processing_master_bias"), 0, 0);
    let master_bias = if let Some(inputs) = &params.bias_inputs {
        info!("Processing master bias...");
        CalibrationImage::new_from_file(inputs, ComputeMethod::Mean)?
    } else {
        CalibrationImage::new_empty()
    };

    check_cancel_status()?;

    info!("Creating process context struct");
    let context = ProcessContext::create_with_calibration_frames(
        &params,
        master_flat,
        master_darkflat,
        master_dark,
        master_bias,
    )?;

    Ok(context)
}

fn frame_sigma_analysis(context: &ProcessContext) -> Result<Vec<FrameRecord>> {
    check_cancel_status()?;

    let frame_count = context.frame_records.len();

    set_task_status(&t!("tasks.frame_analysis"), frame_count, 0);

    let counter = Arc::new(Mutex::new(0));

    let frame_records = frame_analysis_window_size(
        context,
        context.parameters.analysis_window_size,
        move |fr| {
            info!(
                "frame_sigma_analysis(): Frame processed with sigma {}",
                fr.sigma
            );
            // check_cancel_status(&sender);

            let mut c = counter.lock().unwrap();
            *c += 1;
            set_task_status(&t!("tasks.frame_analysis"), frame_count, *c)
        },
    )?;

    Ok(frame_records)
}

fn frame_limiting(context: &ProcessContext) -> Result<Vec<FrameRecord>> {
    check_cancel_status()?;

    let frame_count = context.frame_records.len();

    set_task_status(&t!("tasks.frame_limits"), frame_count, 0);

    let counter = Arc::new(Mutex::new(0));

    let frame_records = frame_limit_determinate(context, move |_fr| {
        info!("frame_limit_determinate(): Frame processed.");
        // check_cancel_status(&sender);

        let mut c = counter.lock().unwrap();
        *c += 1;
        set_task_status(&t!("tasks.frame_limits"), frame_count, *c)
    })?;

    Ok(frame_records)
}

fn frame_rotation(context: &ProcessContext) -> Result<Vec<FrameRecord>> {
    check_cancel_status()?;

    let frame_count = context.frame_records.len();

    set_task_status(&t!("tasks.parallactice_angle"), frame_count, 0);

    let counter = Arc::new(Mutex::new(0));

    let frame_records = frame_rotation_analysis(context, move |fr| {
        info!(
            "Rotation for frame is {} degrees",
            fr.computed_rotation.to_degrees()
        );
        // check_cancel_status(&sender);

        let mut c = counter.lock().unwrap();
        *c += 1;
        set_task_status(&t!("tasks.parallactic_angle"), frame_count, *c)
    })?;

    Ok(frame_records)
}

fn drizzle_stacking(context: &ProcessContext) -> Result<BilinearDrizzle> {
    check_cancel_status()?;

    let frame_count = context.frame_records.len();

    set_task_status(&t!("tasks.stacking"), frame_count, 0);

    let counter = Arc::new(Mutex::new(0));

    let drizzle_output = process_frame_stacking(context, move |_fr| {
        info!("process_frame_stacking(): Frame processed.");
        // check_cancel_status(&sender);

        let mut c = counter.lock().unwrap();
        *c += 1;
        set_task_status(&t!("tasks.stacking"), frame_count, *c)
    })?;

    Ok(drizzle_output)
}
