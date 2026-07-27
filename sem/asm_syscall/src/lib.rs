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


// TODO: your code here.


pub fn read(fd: Fd, buffer: &mut [u8]) -> isize {
    // TODO: your code here.
    unimplemented!();
}


pub fn write(fd: Fd, buffer: &[u8]) -> isize {
    // TODO: your code here.
    unimplemented!();
}


pub fn open(path: &CStr, flags: OpenFlags) -> Fd {
    // TODO: your code here.
    unimplemented!();
}


pub fn close(fd: Fd) -> i32 {
    // TODO: your code here.
    unimplemented!();
}


pub fn pipe(pipefd: &mut [Fd; 2]) -> i32 {
    // TODO: your code here.
    unimplemented!();
}


pub fn dup(oldfd: Fd) -> Fd {
    // TODO: your code here.
    unimplemented!();
}


pub fn fork() -> Pid {
    // TODO: your code here.
    unimplemented!();
}


pub fn execve(path: &CStr, argv: &[CString], envp: &[CString]) -> i32 {
    // TODO: your code here.
    unimplemented!();
}


pub fn exit(status: i32) -> ! {
    // TODO: your code here.
    unimplemented!();
}


pub fn waitpid(pid: Pid, options: WaitOptions) -> (Pid, WaitStatus) {
    // TODO: your code here.
    unimplemented!();
}
