//! Semaphore

use crate::sync::UPSafeCell;
use crate::task::{block_current_and_run_next, current_task, wakeup_task, TaskControlBlock};
use alloc::{collections::VecDeque, sync::Arc, vec::Vec};

/// semaphore structure
pub struct Semaphore {
    /// semaphore inner
    pub inner: UPSafeCell<SemaphoreInner>,
}

pub struct SemaphoreInner {
    pub count: isize,
    pub wait_queue: VecDeque<Arc<TaskControlBlock>>,
    /// track how many units each thread holds from this semaphore: (tid, count)
    pub holders: Vec<(usize, isize)>,
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
                    holders: Vec::new(),
                })
            },
        }
    }

    /// up operation of semaphore
    pub fn up(&self) {
        trace!("kernel: Semaphore::up");
        let mut inner = self.inner.exclusive_access();
        // Remove one unit from the current thread's holdings
        let task = current_task().unwrap();
        let tid = task
            .inner_exclusive_access()
            .res
            .as_ref()
            .expect("Semaphore::up: task res is None")
            .tid;
        if let Some(entry) = inner.holders.iter_mut().find(|(t, _)| *t == tid) {
            entry.1 -= 1;
            if entry.1 <= 0 {
                inner.holders.retain(|(t, _)| *t != tid);
            }
        }
        inner.count += 1;
        if inner.count <= 0 {
            if let Some(task) = inner.wait_queue.pop_front() {
                wakeup_task(task);
            }
        }
    }

    /// down operation of semaphore
    pub fn down(&self) {
        trace!("kernel: Semaphore::down");
        let mut inner = self.inner.exclusive_access();
        inner.count -= 1;
        if inner.count < 0 {
            inner.wait_queue.push_back(current_task().unwrap());
            drop(inner);
            block_current_and_run_next();
            // After waking up, this thread now holds a resource from this semaphore
            let task = current_task().unwrap();
            let tid = task
                .inner_exclusive_access()
                .res
                .as_ref()
                .expect("Semaphore::down: task res is None after wake")
                .tid;
            let mut inner = self.inner.exclusive_access();
            if let Some(entry) = inner.holders.iter_mut().find(|(t, _)| *t == tid) {
                entry.1 += 1;
            } else {
                inner.holders.push((tid, 1));
            }
        } else {
            // Successfully acquired the resource, record the holder
            let task = current_task().unwrap();
            let tid = task
                .inner_exclusive_access()
                .res
                .as_ref()
                .expect("Semaphore::down: task res is None on acquire")
                .tid;
            if let Some(entry) = inner.holders.iter_mut().find(|(t, _)| *t == tid) {
                entry.1 += 1;
            } else {
                inner.holders.push((tid, 1));
            }
        }
    }
}
