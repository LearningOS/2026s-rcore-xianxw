//! Process management syscalls
//!
use alloc::sync::Arc;
use core::mem::size_of;

use crate::{
    config::{PAGE_SIZE, TRAP_CONTEXT_BASE},
    fs::{open_file, OpenFlags},
    mm::{translated_byte_buffer, translated_refmut, translated_str, MapPermission, VirtAddr},
    task::{
        add_task, current_task, current_user_token, exit_current_and_run_next,
        suspend_current_and_run_next,
    },
    timer::get_time_us,
};

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

pub fn sys_exit(exit_code: i32) -> ! {
    trace!("kernel:pid[{}] sys_exit", current_task().unwrap().pid.0);
    exit_current_and_run_next(exit_code);
    panic!("Unreachable in sys_exit!");
}

pub fn sys_yield() -> isize {
    //trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}

pub fn sys_getpid() -> isize {
    trace!("kernel: sys_getpid pid:{}", current_task().unwrap().pid.0);
    current_task().unwrap().pid.0 as isize
}

pub fn sys_fork() -> isize {
    trace!("kernel:pid[{}] sys_fork", current_task().unwrap().pid.0);
    let current_task = current_task().unwrap();
    let new_task = current_task.fork();
    let new_pid = new_task.pid.0;
    // modify trap context of new_task, because it returns immediately after switching
    let trap_cx = new_task.inner_exclusive_access().get_trap_cx();
    // we do not have to move to next instruction since we have done it before
    // for child process, fork returns 0
    trap_cx.x[10] = 0;
    // add new task to scheduler
    add_task(new_task);
    new_pid as isize
}

pub fn sys_exec(path: *const u8) -> isize {
    trace!("kernel:pid[{}] sys_exec", current_task().unwrap().pid.0);
    let token = current_user_token();
    let path = translated_str(token, path);
    if let Some(app_inode) = open_file(path.as_str(), OpenFlags::RDONLY) {
        let all_data = app_inode.read_all();
        let task = current_task().unwrap();
        task.exec(all_data.as_slice());
        0
    } else {
        -1
    }
}

/// If there is not a child process whose pid is same as given, return -1.
/// Else if there is a child process but it is still running, return -2.
pub fn sys_waitpid(pid: isize, exit_code_ptr: *mut i32) -> isize {
    //trace!("kernel: sys_waitpid");
    let task = current_task().unwrap();
    // find a child process

    // ---- access current PCB exclusively
    let mut inner = task.inner_exclusive_access();
    if !inner
        .children
        .iter()
        .any(|p| pid == -1 || pid as usize == p.getpid())
    {
        return -1;
        // ---- release current PCB
    }
    let pair = inner.children.iter().enumerate().find(|(_, p)| {
        // ++++ temporarily access child PCB exclusively
        p.inner_exclusive_access().is_zombie() && (pid == -1 || pid as usize == p.getpid())
        // ++++ release child PCB
    });
    if let Some((idx, _)) = pair {
        let child = inner.children.remove(idx);
        // confirm that child will be deallocated after being removed from children list
        assert_eq!(Arc::strong_count(&child), 1);
        let found_pid = child.getpid();
        // ++++ temporarily access child PCB exclusively
        let exit_code = child.inner_exclusive_access().exit_code;
        // ++++ release child PCB
        *translated_refmut(inner.memory_set.token(), exit_code_ptr) = exit_code;
        found_pid as isize
    } else {
        -2
    }
    // ---- release current PCB automatically
}

/// YOUR JOB: get time with second and microsecond
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TimeVal`] is splitted by two pages ?
pub fn sys_get_time(_ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel:pid[{}] sys_get_time", current_task().unwrap().pid.0);
    if _ts.is_null() {
        return -1;
    }
    let us = get_time_us();
    let timeval = TimeVal {
        sec: us / 1_000_000,
        usec: us % 1_000_000,
    };
    let token = current_user_token();
    let mut user_buffers = translated_byte_buffer(token, _ts as *const u8, size_of::<TimeVal>());
    let timeval_bytes = unsafe {
        core::slice::from_raw_parts((&timeval as *const TimeVal) as *const u8, size_of::<TimeVal>())
    };
    let mut copied = 0;
    for buffer in user_buffers.iter_mut() {
        let n = buffer.len().min(size_of::<TimeVal>() - copied);
        buffer[..n].copy_from_slice(&timeval_bytes[copied..copied + n]);
        copied += n;
        if copied == size_of::<TimeVal>() {
            break;
        }
    }
    0
}

/// YOUR JOB: Implement mmap.
pub fn sys_mmap(_start: usize, _len: usize, _port: usize) -> isize {
    trace!("kernel:pid[{}] sys_mmap", current_task().unwrap().pid.0);
    if _start % PAGE_SIZE != 0 || _len == 0 {
        return -1;
    }
    if _port & !0x7 != 0 || _port & 0x7 == 0 {
        return -1;
    }
    let end = if let Some(end) = _start.checked_add(_len) {
        (end + PAGE_SIZE - 1) / PAGE_SIZE * PAGE_SIZE
    } else {
        return -1;
    };
    if _start >= TRAP_CONTEXT_BASE || end > TRAP_CONTEXT_BASE {
        return -1;
    }

    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access();
    let mut va = _start;
    while va < end {
        if inner.memory_set.translate(VirtAddr::from(va).floor()).is_some() {
            return -1;
        }
        va += PAGE_SIZE;
    }
    let mut map_perm = MapPermission::U;
    if _port & 0x1 != 0 {
        map_perm |= MapPermission::R;
    }
    if _port & 0x2 != 0 {
        map_perm |= MapPermission::W;
    }
    if _port & 0x4 != 0 {
        map_perm |= MapPermission::X;
    }
    inner
        .memory_set
        .insert_framed_area(_start.into(), end.into(), map_perm);
    0
}

/// YOUR JOB: Implement munmap.
pub fn sys_munmap(_start: usize, _len: usize) -> isize {
    trace!("kernel:pid[{}] sys_munmap", current_task().unwrap().pid.0);
    if _start % PAGE_SIZE != 0 || _len == 0 || _len % PAGE_SIZE != 0 {
        return -1;
    }
    let end = if let Some(end) = _start.checked_add(_len) {
        end
    } else {
        return -1;
    };
    if _start >= TRAP_CONTEXT_BASE || end > TRAP_CONTEXT_BASE {
        return -1;
    }

    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access();
    if inner
        .memory_set
        .remove_framed_area(_start.into(), end.into())
    {
        0
    } else if inner
        .memory_set
        .translate(VirtAddr::from(_start).floor())
        .is_some()
    {
        inner
            .memory_set
            .remove_area_with_start_vpn(VirtAddr::from(_start).floor());
        0
    } else {
        -1
    }
}

/// change data segment size
pub fn sys_sbrk(size: i32) -> isize {
    trace!("kernel:pid[{}] sys_sbrk", current_task().unwrap().pid.0);
    if let Some(old_brk) = current_task().unwrap().change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}

/// YOUR JOB: Implement spawn.
/// HINT: fork + exec =/= spawn
pub fn sys_spawn(_path: *const u8) -> isize {
    trace!("kernel:pid[{}] sys_spawn", current_task().unwrap().pid.0);
    let token = current_user_token();
    let path = translated_str(token, _path);
    if let Some(app_inode) = open_file(path.as_str(), OpenFlags::RDONLY) {
        let all_data = app_inode.read_all();
        let current = current_task().unwrap();
        let new_task = current.spawn(all_data.as_slice());
        let new_pid = new_task.pid.0;
        add_task(new_task);
        new_pid as isize
    } else {
        -1
    }
}

// YOUR JOB: Set task priority.
pub fn sys_set_priority(_prio: isize) -> isize {
    trace!(
        "kernel:pid[{}] sys_set_priority prio={}",
        current_task().unwrap().pid.0,
        _prio
    );
    if _prio < 2 {
        return -1;
    }
    let task = current_task().unwrap();
    task.set_priority(_prio);
    _prio
}
