#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

use std::{error::Error, ffi::CString, io, io::prelude::*, str};

use nix::{
    fcntl::{open, OFlag},
    libc::{STDIN_FILENO, STDOUT_FILENO},
    sys::{stat::Mode, wait::waitpid},
    unistd::{close, dup2, execvp, fork, pipe, ForkResult},
};

#[derive(Debug, PartialEq, Eq)]
pub struct Command {
    pub command: Vec<CString>,
    pub stdin: Option<CString>,
    pub stdout: Option<CString>,
}

pub fn parse(line: &str) -> Vec<Command> {
    // TODO: your code here.
    unimplemented!();
}

pub fn main() -> Result<(), Box<dyn Error>> {
    for line in io::stdin().lock().lines() {
        let line = line?;
        let commands = parse(&line);

        // TODO: your code here.
        unimplemented!();
    }

    Ok(())
}
