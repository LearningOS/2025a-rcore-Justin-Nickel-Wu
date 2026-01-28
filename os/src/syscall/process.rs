//! Process management syscalls
use crate::mm::{frame_alloc, translated_byte_buffer, PTEFlags, PageTable, VirtPageNum};
use crate::task::{
    change_program_brk, current_user_token, exit_current_and_run_next, suspend_current_and_run_next,
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
    // check if prot is valid
    if _prot & !0x7 != 0 || _prot & 0x7 == 0 {
        trace!("kernel: sys_mmap failed due to invalid prot!");
        return -1;
    }
    // check if start addr is page-aligned
    if _start & 0xfff != 0 {
        trace!("kernel: sys_mmap failed due to invalid start addr!");
        return -1;
    }
    let mut page_table = PageTable::from_token(current_user_token());
    let page_num = (_len + 4095) / 4096;
    let mut flags = PTEFlags::empty();
    if _prot & 1 != 0 {
        flags |= PTEFlags::R;
    }
    if _prot & 2 != 0 {
        flags |= PTEFlags::W;
    }
    if _prot & 4 != 0 {
        flags |= PTEFlags::X;
    }
    // TODO: need add rollback if any mapping fails in the loop
    for i in 0..page_num {
        let current_page_num = VirtPageNum::from(_start + (i << 12));
        // check if the addr is already mapped
        if let Some(pte) = page_table.translate(current_page_num) {
            if pte.is_valid() {
                trace!("kernel: sys_mmap failed due to addr already mapped!");
                return -1;
            }
        }
        // check if frame allocation fails
        let frame = frame_alloc();
        if frame.is_none() {
            trace!("kernel: sys_mmap failed due to frame alloc failure!");
            return -1;
        }
        let ppn = frame.unwrap().ppn;
        page_table.map(current_page_num, ppn, flags);
    }
    1
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
