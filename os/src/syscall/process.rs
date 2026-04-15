//! Process management syscalls


use crate::config::PAGE_SIZE;
use crate::mm::page_table::translated_byte_buffer;
use crate::mm::{right_read, right_write};
use crate::task::{change_program_brk,get_current_syscall_count, exit_current_and_run_next, suspend_current_and_run_next, current_user_token, mmap_current, munmap_current};

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// task exits and submit an exit code
pub fn sys_exit(_exit_code: i32) -> ! {
    trace!("kernel: sys_exit");
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}

/// YOUR JOB: get time with second and microsecond
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TimeVal`] is splitted by two pages ?
pub fn sys_get_time(ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    let us = crate::timer::get_time_us();
    let timeval = TimeVal {
    sec: us / 1_000_000,
    usec: us % 1_000_000,
    };

    let bytes = unsafe {
        core::slice::from_raw_parts(
            (&timeval as *const TimeVal) as *const u8,
            core::mem::size_of::<TimeVal>(),
        )
    };

    let mut offset = 0;
    for dst in translated_byte_buffer(current_user_token(), ts as *const u8, core::mem::size_of::<TimeVal>()) {
        let n = core::cmp::min(dst.len(), bytes.len() - offset);
        dst[..n].copy_from_slice(&bytes[offset..offset + n]);
        offset += n;
    }

    0
}

/// TODO: Finish sys_trace to pass testcases
/// HINT: You might reimplement it with virtual memory management.
pub fn sys_trace(_trace_request: usize, _id: usize, _data: usize) -> isize {
    trace!("kernel: sys_trace");
    match _trace_request{
        0 => {
            let p_tr = _id as *const u8;
            if right_read(p_tr){
                let buf = translated_byte_buffer(current_user_token(), p_tr, 1);
                buf[0][0] as isize
            }else{
            -1
        }
        }
        1 => {
            let p_tr = _id as *const u8;
            if right_write(p_tr){
                let mut buf = translated_byte_buffer(current_user_token(), p_tr, 1);
                buf[0][0] = _data as u8;
                0

            }else{
            -1
        }
        }
        2 => {
    get_current_syscall_count(_id)
}
         _ => {
            -1
        }
    }
}

// YOUR JOB: Implement mmap.
pub fn sys_mmap(_start: usize, _len: usize, _port: usize) -> isize {
    if _start % PAGE_SIZE != 0 || _port & 0x7 == 0 || _port & !0x7 != 0 {
        return -1;
    }
    if _len == 0 {
        return 0;
    }
    if mmap_current(_start, _len, _port) {
        0
    } else {
        -1
    }
}

// YOUR JOB: Implement munmap.
pub fn sys_munmap(_start: usize, _len: usize) -> isize {
    if _start % PAGE_SIZE != 0 || _len % PAGE_SIZE != 0 {
        return -1;
    }
    if _len == 0 {
        return 0;
    }
    if munmap_current(_start, _len) {
        0
    } else {
        -1
    }
}
/// change data segment size
pub fn sys_sbrk(size: i32) -> isize {
    trace!("kernel: sys_sbrk");
    if let Some(old_brk) = change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}
