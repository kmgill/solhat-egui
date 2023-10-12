use crate::taskstatus::*;
use anyhow::Result;
use std::sync::{Arc, Mutex};
use std::{error::Error, fmt};

#[derive(Debug, PartialEq, Eq)]
pub enum CancelStatus {
    NoStatus,        // Keep doing what you're doing...
    CancelRequested, // Request cancel
    Cancelled,       // Task has cancelled
}

#[derive(Debug, PartialEq, Eq)]
pub enum TaskCompletion {
    Cancelled,
    Completed,
    Error(String),
}

impl Error for TaskCompletion {}

impl fmt::Display for TaskCompletion {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Error of type {:?}", self)
    }
}

pub struct CancelContainer {
    pub status: CancelStatus,
}

lazy_static! {
    pub static ref CANCEL_TASK: Arc<Mutex<CancelContainer>> =
        Arc::new(Mutex::new(CancelContainer {
            status: CancelStatus::NoStatus
        }));
}

pub fn set_request_cancel() {
    CANCEL_TASK.lock().unwrap().status = CancelStatus::CancelRequested;
}

pub fn set_task_cancelled() {
    CANCEL_TASK.lock().unwrap().status = CancelStatus::Cancelled;
}

pub fn reset_cancel_status() {
    CANCEL_TASK.lock().unwrap().status = CancelStatus::NoStatus;
}

pub fn is_cancel_requested() -> bool {
    CANCEL_TASK.lock().unwrap().status == CancelStatus::CancelRequested
}

pub fn check_cancel_status() -> Result<TaskCompletion, TaskCompletion> {
    if is_cancel_requested() {
        set_task_cancelled();
        set_task_completed();
        reset_cancel_status();
        warn!("Task cancellation request detected. Stopping progress");
        Err(TaskCompletion::Cancelled)
    } else {
        Ok(TaskCompletion::Completed)
    }
}
