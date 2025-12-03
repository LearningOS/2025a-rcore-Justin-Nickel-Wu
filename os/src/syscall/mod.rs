//! Implementation of syscalls
//!
//! The single entry point to all system calls, [`syscall()`], is called
//! whenever userspace wishes to perform a system call using the `ecall`
//! instruction. In this case, the processor raises an 'Environment call from
//! U-mode' exception, which is handled as one of the cases in
//! [`crate::trap::trap_handler`].
//!
//! For clarity, each single syscall is implemented as its own function, named
//! `sys_` then the name of the syscall. You can find functions like this in
//! submodules, and you should also implement syscalls this way.
/// The total number of syscalls supported
pub const SYSCALL_NUM: usize = SyscallIdx::ThisIsEnd as usize;
/// write syscall
const SYSCALL_WRITE: usize = 64;
/// exit syscall
const SYSCALL_EXIT: usize = 93;
/// yield syscall
const SYSCALL_YIELD: usize = 124;
/// gettime syscall
const SYSCALL_GET_TIME: usize = 169;
/// trace syscall
const SYSCALL_TRACE: usize = 410;

mod fs;
mod process;

use fs::*;
use process::*;
use crate::task::inc_current_task_syscall_count;

/// map syscall id to array index
enum SyscallIdx {
    Write,
    Exit,
    Yield,
    GetTime,
    Trace,
    ThisIsEnd, // just a marker, used to count number of variants
}

/// map syscall id to array index
pub fn syscall_id_to_idx(syscall_id: usize) -> Option<usize> {
    match syscall_id {
        SYSCALL_WRITE => Some(SyscallIdx::Write as usize),
        SYSCALL_EXIT => Some(SyscallIdx::Exit as usize),
        SYSCALL_YIELD => Some(SyscallIdx::Yield as usize),
        SYSCALL_GET_TIME => Some(SyscallIdx::GetTime as usize),
        SYSCALL_TRACE => Some(SyscallIdx::Trace as usize),
        _ => None,
    }
}

/// handle syscall exception with `syscall_id` and other arguments
pub fn syscall(syscall_id: usize, args: [usize; 3]) -> isize {
    inc_current_task_syscall_count(syscall_id);
    match syscall_id {
        SYSCALL_WRITE => sys_write(args[0], args[1] as *const u8, args[2]),
        SYSCALL_EXIT => sys_exit(args[0] as i32),
        SYSCALL_YIELD => sys_yield(),
        SYSCALL_GET_TIME => sys_get_time(args[0] as *mut TimeVal, args[1]),
        SYSCALL_TRACE => sys_trace(args[0], args[1], args[2]),
        _ => panic!("Unsupported syscall_id: {}", syscall_id),
    }
}
