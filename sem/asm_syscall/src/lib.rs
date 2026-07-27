#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

use std::{
    arch::asm,
    ffi::{c_char, CStr, CString},
    ptr,
    vec::Vec,
};


type Fd = i32;
type OpenFlags = i32;
type Pid = i32;
type WaitOptions = i32;
type WaitStatus = i32;


pub  fn read(fd: Fd, buffer: &mut [u8]) -> isize {
    let ret: isize;
    let count  = buffer.len();
     unsafe {
        asm!(
        "syscall",
        in("rax") 0,
        in("rdi") fd,
        in("rsi") buffer.as_mut_ptr(),
        in("rdx") count,
        lateout("rax") ret,
        options(nostack)
        );
    }
    ret
}


pub  fn write(fd: Fd, buffer: &[u8]) -> isize {
    let ret: isize;
    let count  = buffer.len();
     unsafe {
        asm!(
        "syscall",
        in("rax") 1,
        in("rdi") fd,
        in("rsi") buffer.as_ptr(),
        in("rdx") count,
        lateout("rax") ret,
        options(nostack)
        );
    }

    ret
}


pub  fn open(path: &CStr, flags: OpenFlags) -> Fd {
    let fd;
    unsafe {
        asm!(
        "syscall",
        in("rax") 2,
        in("rdi") path.as_ptr(),
        in("rsi") flags,
        lateout("rax") fd,
        options(nostack)
        );
    }

    fd
}


pub  fn close(fd: Fd) -> i32 {
    let ret : i32;
    unsafe {
        asm!(
        "syscall",
        in("rax") 3,
        in("rdi") fd,
        lateout("rax") ret,
        options(nostack)
        );
    }

    ret
}


pub  fn pipe(pipefd: &mut [Fd; 2]) -> i32 {
    let ret : i32;
    unsafe {
        asm!(
        "syscall",
        in("rax") 22,
        in("rdi") pipefd.as_mut_ptr(),
        lateout("rax") ret,
        options(nostack)
        );
    }

    ret
}


pub  fn dup(oldfd: Fd) -> Fd {
    let fd : Fd;
    unsafe {
        asm!(
        "syscall",
        in("rax") 32,
        in("rdi") oldfd,
        lateout("rax") fd,
        options(nostack)
        );
    }

    fd
}


pub  fn fork() -> Pid {
    let pid : Pid;
    unsafe {
        asm!(
        "syscall",
        in("rax") 57,
        lateout("rax") pid,
        options(nostack)
        );
    }

    pid
}


pub  fn execve(path: &CStr, argv: &[CString], envp: &[CString]) -> i32 {
    let mut ret : i32;

    let mut argv_ptrs: Vec<*const c_char> = argv.iter().map(|s| s.as_ptr()).collect();
    argv_ptrs.push(ptr::null());

    let mut envp_ptrs: Vec<*const c_char> = envp.iter().map(|s| s.as_ptr()).collect();
    envp_ptrs.push(ptr::null());
    unsafe {
        asm!(
        "syscall",
        in("rax") 59,
        in("rdi") path.as_ptr(),
        in("rsi") argv_ptrs.as_ptr(),
        in("rdx") envp_ptrs.as_ptr(),
        lateout("rax") ret,
        options(nostack)
        );
    }

    ret
}


pub  fn exit(status: i32) -> ! {
    unsafe {
        asm!(
        "syscall",
        in("rax") 60,
        in("rdi") status,
        options(noreturn),
        )
    }
}


pub  fn waitpid(pid: Pid, options: WaitOptions) -> (Pid, WaitStatus) {
    let waitpid : WaitStatus;
    let ret:Pid;
    unsafe {
        asm!(
        "syscall",
        in("rax") 61,
        in("rdi") pid,
        in("rsi") options,
        lateout("rax") ret,
        lateout("rsi") waitpid,
        options(nostack),
        );
    }

    (ret, waitpid)
}
