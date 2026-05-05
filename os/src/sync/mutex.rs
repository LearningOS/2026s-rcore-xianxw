//! Mutex (spin-like and blocking(sleep))

use super::UPSafeCell;
use crate::task::TaskControlBlock;
use crate::task::{block_current_and_run_next, suspend_current_and_run_next};
use crate::task::{current_task, wakeup_task};
use alloc::{collections::VecDeque, sync::Arc, vec::Vec};

/// Mutex trait
pub trait Mutex: Sync + Send {
    /// Lock the mutex
    fn lock(&self);
    /// Unlock the mutex
    fn unlock(&self);
    /// Check if the mutex is locked
    fn is_locked(&self) -> bool;
    /// Get the TIDs of threads waiting on this mutex
    fn get_waiting_tids(&self) -> Vec<usize>;
    /// Get the TID of the thread holding this mutex (if any)
    fn get_holder_tid(&self) -> Option<usize>;
}

/// Spinlock Mutex struct
pub struct MutexSpin {
    locked: UPSafeCell<bool>,
}

impl MutexSpin {
    /// Create a new spinlock mutex
    pub fn new() -> Self {
        Self {
            locked: unsafe { UPSafeCell::new(false) },
        }
    }
}

impl Mutex for MutexSpin {
    /// Lock the spinlock mutex
    fn lock(&self) {
        trace!("kernel: MutexSpin::lock");
        loop {
            let mut locked = self.locked.exclusive_access();
            if *locked {
                drop(locked);
                suspend_current_and_run_next();
                continue;
            } else {
                *locked = true;
                return;
            }
        }
    }

    fn unlock(&self) {
        trace!("kernel: MutexSpin::unlock");
        let mut locked = self.locked.exclusive_access();
        *locked = false;
    }

    fn is_locked(&self) -> bool {
        *self.locked.exclusive_access()
    }

    fn get_waiting_tids(&self) -> Vec<usize> {
        Vec::new()
    }

    fn get_holder_tid(&self) -> Option<usize> {
        None
    }
}

/// Blocking Mutex struct
pub struct MutexBlocking {
    inner: UPSafeCell<MutexBlockingInner>,
}

pub struct MutexBlockingInner {
    locked: bool,
    holder: Option<usize>,
    wait_queue: VecDeque<Arc<TaskControlBlock>>,
}

impl MutexBlocking {
    /// Create a new blocking mutex
    pub fn new() -> Self {
        trace!("kernel: MutexBlocking::new");
        Self {
            inner: unsafe {
                UPSafeCell::new(MutexBlockingInner {
                    locked: false,
                    holder: None,
                    wait_queue: VecDeque::new(),
                })
            },
        }
    }
}

impl Mutex for MutexBlocking {
    /// lock the blocking mutex
    fn lock(&self) {
        trace!("kernel: MutexBlocking::lock");
        let mut mutex_inner = self.inner.exclusive_access();
        if mutex_inner.locked {
            mutex_inner.wait_queue.push_back(current_task().unwrap());
            drop(mutex_inner);
            block_current_and_run_next();
            // After waking up, this task now holds the lock
            let tid = current_task()
                .unwrap()
                .inner_exclusive_access()
                .res
                .as_ref()
                .unwrap()
                .tid;
            let mut mutex_inner = self.inner.exclusive_access();
            mutex_inner.holder = Some(tid);
        } else {
            mutex_inner.locked = true;
            let tid = current_task()
                .unwrap()
                .inner_exclusive_access()
                .res
                .as_ref()
                .unwrap()
                .tid;
            mutex_inner.holder = Some(tid);
        }
    }

    /// unlock the blocking mutex
    fn unlock(&self) {
        trace!("kernel: MutexBlocking::unlock");
        let mut mutex_inner = self.inner.exclusive_access();
        assert!(mutex_inner.locked);
        if let Some(waking_task) = mutex_inner.wait_queue.pop_front() {
            // Transfer the lock to the woken task
            wakeup_task(waking_task);
        } else {
            mutex_inner.locked = false;
            mutex_inner.holder = None;
        }
    }

    fn is_locked(&self) -> bool {
        self.inner.exclusive_access().locked
    }

    fn get_waiting_tids(&self) -> Vec<usize> {
        let inner = self.inner.exclusive_access();
        inner
            .wait_queue
            .iter()
            .map(|task| {
                task.inner_exclusive_access()
                    .res
                    .as_ref()
                    .unwrap()
                    .tid
            })
            .collect()
    }

    fn get_holder_tid(&self) -> Option<usize> {
        self.inner.exclusive_access().holder
    }
}
