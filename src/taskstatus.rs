use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub enum TaskStatus {
    TaskPercentage(String, usize, usize),
}

#[derive(Default, Clone)]
pub struct TaskStatusContainer {
    pub status: Option<TaskStatus>,
}

lazy_static! {
    static ref TASK_STATUS_QUEUE: Arc<Mutex<TaskStatusContainer>> =
        Arc::new(Mutex::new(TaskStatusContainer::default()));
}

pub fn is_task_running() -> bool {
    if let Ok(status) = TASK_STATUS_QUEUE.lock() {
        status.status.is_some()
    } else {
        false
    }
}

pub fn get_task_status() -> Option<TaskStatus> {
    if let Ok(status) = TASK_STATUS_QUEUE.lock() {
        status.status.clone()
    } else {
        None
    }
}

pub fn set_task_status(task_name: &str, num_parts: usize, progress: usize) {
    // let mut status = TASK_STATUS_QUEUE.lock().unwrap();
    if let Ok(mut status) = TASK_STATUS_QUEUE.lock() {
        status.status = Some(TaskStatus::TaskPercentage(
            task_name.to_owned(),
            num_parts,
            progress,
        ));
    }
    // sender
    //     .send(TaskStatusContainer {
    //         status: Some(TaskStatus::TaskPercentage(
    //             task_name.to_owned(),
    //             num_parts,
    //             progress,
    //         )),
    //     })
    //     .expect("Failed to sent task status");
}

pub fn set_task_completed() {
    if let Ok(mut status) = TASK_STATUS_QUEUE.lock() {
        status.status = None;
    }
    // sender
    //     .send(TaskStatusContainer { status: None })
    //     .expect("Failed to sent task status");
}
