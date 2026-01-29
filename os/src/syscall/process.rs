//! Process management syscalls
//!
use alloc::sync::Arc;

use crate::{
    config::PAGE_SIZE,
    fs::{open_file, OpenFlags},
    mm::{translated_byte_buffer, translated_refmut, translated_str, MapPermission, VirtAddr},
    task::{
        add_task, current_check_page_mapped, current_map_pages, current_task, current_unmap_pages,
        current_user_token, exit_current_and_run_next, suspend_current_and_run_next,
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
    trace!(
        "kernel::pid[{}] sys_waitpid [{}]",
        current_task().unwrap().pid.0,
        pid
    );
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

/// YOUR JOB: Implement mmap.
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
        let vpn = VirtAddr::from(addr).floor();
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

/// YOUR JOB: Implement munmap.
pub fn sys_munmap(_start: usize, _len: usize) -> isize {
    // 检查地址是否对其
    if _start & 0xfff != 0 {
        trace!("kernel: sys_munmap failed due to invalid start addr!");
        return -1;
    }
    // 检查是否虚拟地址已经映射
    let page_num = (_len + PAGE_SIZE - 1) / PAGE_SIZE;
    for i in 0..page_num {
        let addr = _start + i * PAGE_SIZE;
        let vpn = VirtAddr::from(addr).floor();
        if !current_check_page_mapped(vpn) {
            trace!("kernel: sys_munmap failed! VPN {:x} is not mapped!", vpn.0);
            return -1;
        }
    }
    // 进行解除映射
    current_unmap_pages(_start, page_num);
    0
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
    let path = translated_str(current_user_token(), _path);
    if let Some(app_inode) = open_file(path.as_str(), OpenFlags::RDONLY) {
        let all_data = app_inode.read_all();
        let new_task = current_task().unwrap().spawn(all_data.as_slice());
        let new_task_pid = new_task.getpid() as isize;
        add_task(new_task);
        new_task_pid
    } else {
        -1
    }
}

// YOUR JOB: Set task priority.
pub fn sys_set_priority(_prio: isize) -> isize {
    trace!(
        "kernel:pid[{}] sys_set_priority",
        current_task().unwrap().pid.0
    );
    if _prio >= 2 {
        let task = current_task().unwrap();
        let mut inner = task.inner_exclusive_access();
        inner.set_priority(_prio as usize);
        _prio
    } else {
        -1
    }
}
