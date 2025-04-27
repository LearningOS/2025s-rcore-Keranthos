//! Semaphore

use crate::sync::UPSafeCell;
use crate::task::{
    block_current_and_run_next, current_process, current_task, wakeup_task, TaskControlBlock,
};
use alloc::{collections::VecDeque, sync::Arc};

/// semaphore structure
pub struct Semaphore {
    /// semaphore inner
    pub inner: UPSafeCell<SemaphoreInner>,
}

pub struct SemaphoreInner {
    pub count: isize,
    pub wait_queue: VecDeque<Arc<TaskControlBlock>>,
}

impl Semaphore {
    /// Create a new semaphore
    pub fn new(res_count: usize) -> Self {
        trace!("kernel: Semaphore::new");
        Self {
            inner: unsafe {
                UPSafeCell::new(SemaphoreInner {
                    count: res_count as isize,
                    wait_queue: VecDeque::new(),
                })
            },
        }
    }

    /// up operation of semaphore
    pub fn up(&self) {
        trace!("kernel: Semaphore::up");
        let mut inner = self.inner.exclusive_access();
        inner.count += 1;
        if inner.count <= 0 {
            if let Some(task) = inner.wait_queue.pop_front() {
                wakeup_task(task);
            }
        }
    }

    /// down operation of semaphore
    pub fn down(&self, sem_id: usize) {
        trace!("kernel: Semaphore::down");
        let mut inner = self.inner.exclusive_access();
        inner.count -= 1;
        let tid = current_task().unwrap().get_pid();
        /* let mut s_inner = current_process().inner_exclusive_access()
        .deadlock_matrix
        .semaphore_inner
        .exclusive_access(); */
        if inner.count < 0 {
            inner.wait_queue.push_back(current_task().unwrap());
            current_process()
                .inner_exclusive_access()
                .deadlock_matrix
                .semaphore_inner
                .exclusive_access()
                .need[tid][sem_id] += 1;
            drop(inner);
            block_current_and_run_next();
        } else {
            current_process()
                .inner_exclusive_access()
                .deadlock_matrix
                .semaphore_inner
                .exclusive_access()
                .need[tid][sem_id] = 0;
            current_process()
                .inner_exclusive_access()
                .deadlock_matrix
                .semaphore_inner
                .exclusive_access()
                .allocation[tid][sem_id] += 1;
        }
    }
}
