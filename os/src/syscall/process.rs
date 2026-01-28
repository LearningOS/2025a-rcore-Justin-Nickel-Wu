//! Process management syscalls
use crate::config::PAGE_SIZE;
use crate::mm::{translated_byte_buffer, MapPermission, VirtAddr, VirtPageNum};
use crate::task::{
    change_program_brk, current_check_page_mapped, current_map_pages, current_user_token,
    exit_current_and_run_next, suspend_current_and_run_next,
};
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

    // 获取当前用户页表 token
    let token = current_user_token();

    // 读取当前时间（微秒）
    let us = get_time_us();
    let tv = TimeVal {
        sec: us / 1_000_000,
        usec: us % 1_000_000,
    };

    // 把 TimeVal 当成字节切片（内核虚拟地址）
    let src = unsafe {
        core::slice::from_raw_parts(
            (&tv as *const TimeVal) as *const u8,
            core::mem::size_of::<TimeVal>(),
        )
    };

    // 将用户虚拟地址翻译成可写的内核字节缓冲（可能跨页）
    let mut dsts: alloc::vec::Vec<&mut [u8]> =
        translated_byte_buffer(token, _ts as *const u8, core::mem::size_of::<TimeVal>());

    // 分段拷贝（安全处理跨页）
    let mut offset = 0usize;
    for dst in dsts.iter_mut() {
        let len = dst.len();
        dst.copy_from_slice(&src[offset..offset + len]);
        offset += len;
    }

    0
}

/// TODO: Finish sys_trace to pass testcases
/// HINT: You might reimplement it with virtual memory management.
pub fn sys_trace(_trace_request: usize, _id: usize, _data: usize) -> isize {
    trace!("kernel: sys_trace");
    -1
}

// YOUR JOB: Implement mmap.
pub fn sys_mmap(_start: usize, _len: usize, _prot: usize) -> isize {
    trace!("kernel: sys_mmap");
    // 检查prot合法性
    if _prot & !0x7 != 0 || _prot & 0x7 == 0 {
        trace!("kernel: sys_mmap failed due to invalid prot!");
        return -1;
    }
    // 检查地址是否对其
    if _start & 0xfff != 0 {
        trace!("kernel: sys_mmap failed due to invalid start addr!");
        return -1;
    }
    // 检查是否虚拟地址是否已经映射
    let page_num = (_len + PAGE_SIZE - 1) / PAGE_SIZE;
    for i in 0..page_num {
        let addr = _start + i * PAGE_SIZE;
        let vpn = VirtPageNum::from(VirtAddr::from(addr));
        if current_check_page_mapped(vpn) {
            trace!(
                "kernel: sys_mmap failed! VPN {:x} is already mapped!",
                vpn.0
            );
            return -1;
        }
    }
    let mut flags = MapPermission::U;
    if _prot & 0x1 != 0 {
        flags |= MapPermission::R;
    }
    if _prot & 0x2 != 0 {
        flags |= MapPermission::W;
    }
    if _prot & 0x4 != 0 {
        flags |= MapPermission::X;
    }
    // 进行映射
    current_map_pages(_start, page_num, flags);
    0
}

// YOUR JOB: Implement munmap.
pub fn sys_munmap(_start: usize, _len: usize) -> isize {
    trace!("kernel: sys_munmap NOT IMPLEMENTED YET!");
    -1
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
