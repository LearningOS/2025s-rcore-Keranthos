//! Process management syscalls
use crate::task::{change_program_brk, exit_current_and_run_next, suspend_current_and_run_next, current_user_token, get_syscall_counts, map_memory, unmap_memory};
use crate::mm::{ translated_byte_buffer, check_user_accessible, MapPermission, VirtAddr };
use crate::timer::get_time_us;

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
pub fn sys_get_time(_ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    let us = get_time_us();
    let time_val = TimeVal { sec: us / 1000000, usec: us % 1000000 };

    let bytes: &[u8] = unsafe {
        core::slice::from_raw_parts(
            &time_val as *const _ as *const u8,
            core::mem::size_of::<TimeVal>(),
        )
    };
    let buffers = translated_byte_buffer(
        current_user_token(),
        _ts as *const u8,
        bytes.len(),
    );
    let mut offset = 0;
    for buf in buffers {
        let len = buf.len().min(bytes.len() - offset);
        buf[..len].copy_from_slice(&bytes[offset..offset + len]);
        offset += len;
    }
    0
}

/// TODO: Finish sys_trace to pass testcases
/// HINT: You might reimplement it with virtual memory management.
pub fn sys_trace(_trace_request: usize, _id: usize, _data: usize) -> isize {
    trace!("kernel: sys_trace");
    match _trace_request {
        0 => {
            read_byte(_id)
        }
        1 => {
            write_byte(_id, _data)
        }
        2 => {
            get_syscall_counts(_id)
        }
        _ => {
            return -1;
        }
    }
}

fn read_byte(id: usize) -> isize {
    if check_user_accessible(current_user_token(), id, false) == false {
        return -1 as isize;
    }
    let buffer = translated_byte_buffer(current_user_token(), id as *const u8, 1);
    buffer[0][0] as isize
}

fn write_byte(id: usize, data: usize) -> isize {
    if check_user_accessible(current_user_token(), id, true) == false {
        return -1 as isize;
    }
    let mut buffer = translated_byte_buffer(current_user_token(), id as *const u8, 1);
    buffer[0][0] = data as u8;
    0
}

// YOUR JOB: Implement mmap.
pub fn sys_mmap(_start: usize, _len: usize, _port: usize) -> isize {
    trace!("kernel: sys_mmap NOT IMPLEMENTED YET!");
    let start_va = VirtAddr::from(_start);
    let end_va = VirtAddr::from(_start + _len);
    if !start_va.aligned() {
        return -1;
    }
    if _port & !0x7 != 0 {
        return -1;
    }
    if _port & 0x7 == 0 {
        return -1;
    }
    // let page_count = (len + 511) / 512;
    let mut mp = MapPermission::U;
    if ((_port >> 0) & 1) == 1 { mp.insert(MapPermission::R); }
    if ((_port >> 1) & 1) == 1 { mp.insert(MapPermission::W); }
    if ((_port >> 2) & 1) == 1 { mp.insert(MapPermission::X); }

    map_memory( start_va, end_va, mp )
}

// YOUR JOB: Implement munmap.
pub fn sys_munmap(_start: usize, _len: usize) -> isize {
    trace!("kernel: sys_munmap NOT IMPLEMENTED YET!");
    let start_va = VirtAddr::from(_start);
    let end_va = VirtAddr::from(_start + _len);
    if !start_va.aligned() {
        return -1;
    }

    unmap_memory( start_va, end_va )
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
