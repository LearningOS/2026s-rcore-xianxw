use crate::sync::{Condvar, Mutex, MutexBlocking, MutexSpin, Semaphore};
use crate::task::{current_process, current_task, suspend_current_and_run_next};
use crate::timer::get_time_ms;
use alloc::sync::Arc;
use alloc::vec;
/// sleep syscall
///
/// Busy-wait by yielding until the specified time has elapsed.
/// We use `suspend_current_and_run_next()` (yield) instead of `block_current_and_run_next()`
/// because timer interrupts cannot be taken while the kernel idle loop is running
/// (sstatus.SIE is cleared by hardware on trap entry), so `add_timer` + `block` doesn't work.
pub fn sys_sleep(ms: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_sleep",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let expire_ms = get_time_ms() + ms;
    while get_time_ms() < expire_ms {
        suspend_current_and_run_next();
    }
    0
}
/// mutex create syscall
pub fn sys_mutex_create(blocking: bool) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_mutex_create",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mutex: Option<Arc<dyn Mutex>> = if !blocking {
        Some(Arc::new(MutexSpin::new()))
    } else {
        Some(Arc::new(MutexBlocking::new()))
    };
    let mut process_inner = process.inner_exclusive_access();
    if let Some(id) = process_inner
        .mutex_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.mutex_list[id] = mutex;
        id as isize
    } else {
        process_inner.mutex_list.push(mutex);
        process_inner.mutex_list.len() as isize - 1
    }
}
/// mutex lock syscall
pub fn sys_mutex_lock(mutex_id: usize) -> isize {
    let current_tid = current_task()
        .unwrap()
        .inner_exclusive_access()
        .res
        .as_ref()
        .unwrap()
        .tid;
    trace!(
        "kernel:pid[{}] tid[{}] sys_mutex_lock",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let deadlock_detect = process_inner.enable_deadlock_detect;
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());

    // If deadlock detection is enabled and the mutex is locked, check for deadlock
    if deadlock_detect && mutex.is_locked() {
        let task_count = process_inner.tasks.len();
        let result = check_mutex_deadlock(
            &process_inner.mutex_list,
            task_count,
            current_tid,
            mutex_id,
        );
        drop(process_inner);
        drop(process);
        if !result {
            // Deadlock detected, reject the lock request
            warn!("kernel: deadlock detected on mutex {}", mutex_id);
            return -0xDEAD;
        }
        mutex.lock();
    } else {
        drop(process_inner);
        drop(process);
        mutex.lock();
    }
    0
}
/// mutex unlock syscall
pub fn sys_mutex_unlock(mutex_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_mutex_unlock",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    drop(process_inner);
    drop(process);
    mutex.unlock();
    0
}
/// semaphore create syscall
pub fn sys_semaphore_create(res_count: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_semaphore_create",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let id = if let Some(id) = process_inner
        .semaphore_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.semaphore_list[id] = Some(Arc::new(Semaphore::new(res_count)));
        id
    } else {
        process_inner
            .semaphore_list
            .push(Some(Arc::new(Semaphore::new(res_count))));
        process_inner.semaphore_list.len() - 1
    };
    id as isize
}
/// semaphore up syscall
pub fn sys_semaphore_up(sem_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_semaphore_up",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let sem = Arc::clone(process_inner.semaphore_list[sem_id].as_ref().unwrap());
    drop(process_inner);
    sem.up();
    0
}
/// semaphore down syscall
pub fn sys_semaphore_down(sem_id: usize) -> isize {
    let current_tid = current_task()
        .unwrap()
        .inner_exclusive_access()
        .res
        .as_ref()
        .unwrap()
        .tid;
    trace!(
        "kernel:pid[{}] tid[{}] sys_semaphore_down",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let deadlock_detect = process_inner.enable_deadlock_detect;

    // If deadlock detection is enabled and the semaphore count is <= 0,
    // check for deadlock before blocking
    if deadlock_detect {
        let sem_count = process_inner.semaphore_list[sem_id]
            .as_ref()
            .unwrap()
            .inner
            .exclusive_access()
            .count;
        if sem_count <= 0 {
            let task_count = process_inner.tasks.len();
            let result = check_semaphore_deadlock(
                &process_inner.semaphore_list,
                task_count,
                current_tid,
                sem_id,
            );
            if !result {
                drop(process_inner);
                drop(process);
                return -0xDEAD;
            }
        }
    }

    let sem = Arc::clone(process_inner.semaphore_list[sem_id].as_ref().unwrap());
    drop(process_inner);
    drop(process);
    sem.down();
    0
}
/// condvar create syscall
pub fn sys_condvar_create() -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_condvar_create",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let id = if let Some(id) = process_inner
        .condvar_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.condvar_list[id] = Some(Arc::new(Condvar::new()));
        id
    } else {
        process_inner
            .condvar_list
            .push(Some(Arc::new(Condvar::new())));
        process_inner.condvar_list.len() - 1
    };
    id as isize
}
/// condvar signal syscall
pub fn sys_condvar_signal(condvar_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_condvar_signal",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let condvar = Arc::clone(process_inner.condvar_list[condvar_id].as_ref().unwrap());
    drop(process_inner);
    condvar.signal();
    0
}
/// condvar wait syscall
pub fn sys_condvar_wait(condvar_id: usize, mutex_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_condvar_wait",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let condvar = Arc::clone(process_inner.condvar_list[condvar_id].as_ref().unwrap());
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    drop(process_inner);
    condvar.wait(mutex);
    0
}
/// enable deadlock detection syscall
pub fn sys_enable_deadlock_detect(enabled: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_enable_deadlock_detect({})",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid,
        enabled
    );
    if enabled != 0 && enabled != 1 {
        return -1;
    }
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    process_inner.enable_deadlock_detect = enabled == 1;
    0
}

/// Check if granting a mutex lock request would lead to a deadlock.
/// Returns true if the state is safe (no deadlock), false if deadlock would occur.
fn check_mutex_deadlock(
    mutex_list: &[Option<Arc<dyn Mutex>>],
    task_count: usize,
    requesting_tid: usize,
    requesting_mutex_id: usize,
) -> bool {
    let num_mutexes = mutex_list.len();
    if num_mutexes == 0 || task_count == 0 {
        return true;
    }

    // Build the current allocation and need matrices
    // Available[j] = 1 if mutex j is unlocked, else 0
    // Allocation[i][j] = 1 if thread i holds mutex j
    // Need[i][j] = 1 if thread i is waiting for mutex j

    let mut available = vec![0isize; num_mutexes];
    let mut allocation = vec![vec![0isize; num_mutexes]; task_count];
    let mut need = vec![vec![0isize; num_mutexes]; task_count];

    // For each mutex, determine its state
    for (j, item) in mutex_list.iter().enumerate() {
        if let Some(mutex) = item {
            if !mutex.is_locked() {
                available[j] = 1;
            }
            // Get holder
            if let Some(holder_tid) = mutex.get_holder_tid() {
                if holder_tid < task_count {
                    allocation[holder_tid][j] = 1;
                }
            }
            // Get waiting threads
            for &waiting_tid in &mutex.get_waiting_tids() {
                if waiting_tid < task_count {
                    need[waiting_tid][j] = 1;
                }
            }
        } else {
            available[j] = 1; // nonexistent mutex is always available
        }
    }

    // Simulate the pending request: requesting_tid wants requesting_mutex_id
    // If the mutex is already available, no deadlock possible
    if available[requesting_mutex_id] == 1 {
        return true;
    }

    // In the "what if we block" scenario:
    // - requesting_tid will be added to wait queue of requesting_mutex_id
    // - So Need[requesting_tid][requesting_mutex_id] = 1
    need[requesting_tid][requesting_mutex_id] = 1;

    // Run the Banker's safety algorithm
    let mut work = available.clone();
    let mut finish = vec![false; task_count];

    loop {
        let mut found = false;
        for i in 0..task_count {
            if !finish[i] {
                // Check if Need[i] <= Work
                let mut need_le_work = true;
                for j in 0..num_mutexes {
                    if need[i][j] > work[j] {
                        need_le_work = false;
                        break;
                    }
                }
                if need_le_work {
                    // Thread i can finish, release its resources
                    for j in 0..num_mutexes {
                        work[j] += allocation[i][j];
                    }
                    finish[i] = true;
                    found = true;
                }
            }
        }
        if !found {
            break;
        }
    }

    // If all threads can finish, state is safe
    finish.iter().all(|&f| f)
}

/// Check if granting a semaphore down request would lead to a deadlock.
/// Uses wait-for graph cycle detection with banker's algorithm safety check.
///
/// Returns true if safe (no deadlock), false if deadlock detected.
fn check_semaphore_deadlock(
    semaphore_list: &[Option<Arc<Semaphore>>],
    task_count: usize,
    requesting_tid: usize,
    requesting_sem_id: usize,
) -> bool {
    let num_sems = semaphore_list.len();
    if num_sems == 0 || task_count == 0 {
        return true;
    }

    // If the semaphore has available resources, no deadlock
    let sem = semaphore_list[requesting_sem_id].as_ref().unwrap();
    let sem_inner = sem.inner.exclusive_access();
    let count = sem_inner.count;
    if count > 0 {
        return true;
    }

    // Check for self-deadlock: requesting thread already holds this semaphore
    let requesting_holds = sem_inner.holders.iter().any(|&(tid, _)| tid == requesting_tid);
    if requesting_holds {
        // Self-deadlock: requesting thread already holds this semaphore
        return false;
    }

    // If the semaphore has no holders and count <= 0, it's being used as a
    // synchronization primitive (e.g., barrier), not as a resource semaphore.
    // In this case, an external thread (not blocked on this semaphore) can signal
    // it, so we should not flag it as a deadlock.
    if sem_inner.holders.is_empty() && count <= 0 {
        return true;
    }
    drop(sem_inner);

    // Build wait-for graph and detect cycles using banker's algorithm
    // allocation[i][j] = resources of sem j held by thread i
    // need[i][j] = 1 if thread i is waiting for sem j
    let mut allocation = vec![vec![0isize; num_sems]; task_count];
    let mut need = vec![vec![0isize; num_sems]; task_count];

    for (j, item) in semaphore_list.iter().enumerate() {
        if let Some(sem) = item {
            let inner = sem.inner.exclusive_access();
            for &(tid, cnt) in &inner.holders {
                if tid < task_count {
                    allocation[tid][j] = cnt;
                }
            }
            for task in inner.wait_queue.iter() {
                let tid = task
                    .inner_exclusive_access()
                    .res
                    .as_ref()
                    .expect("check_semaphore_deadlock: waiter res is None")
                    .tid;
                if tid < task_count {
                    need[tid][j] = 1;
                }
            }
        }
    }

    // Add pending request
    need[requesting_tid][requesting_sem_id] = 1;

    // Run the banker's safety algorithm
    let mut work = vec![0isize; num_sems];
    // Work = available resources from semaphores
    for (j, item) in semaphore_list.iter().enumerate() {
        if let Some(sem) = item {
            work[j] = sem.inner.exclusive_access().count.max(0);
        }
    }

    let mut finish = vec![false; task_count];

    loop {
        let mut found = false;
        for i in 0..task_count {
            if !finish[i] {
                let mut need_le_work = true;
                for j in 0..num_sems {
                    if need[i][j] > work[j] {
                        need_le_work = false;
                        break;
                    }
                }
                if need_le_work {
                    for j in 0..num_sems {
                        work[j] += allocation[i][j];
                    }
                    finish[i] = true;
                    found = true;
                }
            }
        }
        if !found {
            break;
        }
    }

    finish.iter().all(|&f| f)
}
