//! Process management syscalls
use crate::task::{
    change_program_brk, exit_current_and_run_next, suspend_current_and_run_next, 
    current_user_token, TASK_MANAGER, trace_read, trace_write
};
use crate::timer::get_time_us;
use crate::mm::translated_byte_buffer;
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
    // trace!("kernel: sys_get_time");
    let us = get_time_us();
    let sec = (us / 1_000_000) as usize;
    let usec = (us % 1_000_000) as usize;

    let token = current_user_token();
    let size = core::mem::size_of::<TimeVal>();

    let mut bufs = translated_byte_buffer(token, ts as *const u8, size);

    let sec_bytes = sec.to_ne_bytes();
    let usec_bytes = usec.to_ne_bytes();
    let usize_bytes = core::mem::size_of::<usize>();

    let mut written = 0usize;
    for chunk in bufs.iter_mut() {
        for b in chunk.iter_mut() {
            if written < usize_bytes {
                *b = sec_bytes[written];
            } else if written < 2 * usize_bytes {
                *b = usec_bytes[written - usize_bytes];
            } else {
                break;
            }
            written += 1;
            if written >= 2 * usize_bytes {
                break;
            }
        }
        if written >= 2 * usize_bytes {
            break;
        }
    }

    0
}

/// TODO: Finish sys_trace to pass testcases
/// HINT: You might reimplement it with virtual memory management.
pub fn sys_trace(trace_request: usize, id: usize, data: usize) -> isize {
    trace!("kernel: sys_trace");
    match trace_request {
        0 => trace_read(id),
        1 => trace_write(id, data),
        2 => TASK_MANAGER.get_syscall_count(id) as isize,
        _ => -1,
    }
}

// YOUR JOB: Implement mmap.
pub fn sys_mmap(start: usize, len: usize, port: usize) -> isize {
    trace!("kernel: sys_mmap NOT IMPLEMENTED YET!");
    TASK_MANAGER.map_memory(start, len, port)
}

// YOUR JOB: Implement munmap.
pub fn sys_munmap(start: usize, len: usize) -> isize {
    trace!("kernel: sys_munmap");
    TASK_MANAGER.unmap_memory(start, len)
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
