#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

use std::{error::Error, ffi::CString, io, io::prelude::*, str};
use std::os::fd::RawFd;
use std::ptr::null;
use nix::{fcntl::{open, OFlag}, libc, libc::{STDIN_FILENO, STDOUT_FILENO}, sys::{stat::Mode, wait::waitpid}, unistd::{close, dup2, execvp, fork, pipe, ForkResult}};
use nix::libc::{read, uinput_ff_upload, EOF};

#[derive(Debug, PartialEq, Eq)]
pub struct Command {
    pub command: Vec<CString>,
    pub stdin: Option<CString>,
    pub stdout: Option<CString>,
}

fn read_from_fd(fd : RawFd) -> Result<Vec<u8>, io::Error>
{
    let mut buffer = [0u8; 1024];
    let mut output = Vec::new();

    loop {
        let result = unsafe {
            read(fd, buffer.as_mut_ptr() as *mut libc::c_void, buffer.len())
        };

        if result == -1
        {
            return Err(io::Error::last_os_error());
        } else if result == 0
        {
            break;
        } else {
            let bytes_read = result as usize;
            output.extend_from_slice(&buffer[..bytes_read]);
        }
    }

    Ok(output)
}
pub fn parse(line: &str) -> Vec<Command>
{
    let mut commands : Vec<Command> = Vec::new();
    let args : Vec<&str> = line.split_whitespace().collect();
    let mut command : Vec<CString> = Vec::new();
    let mut stdin : Option<CString> = None;
    let mut stdout : Option<CString> = None;
    let mut first : bool = true;

    let mut iter = args.iter();

    while let Some(&arg) = iter.next()
    {
        if (arg.starts_with('>'))
        {
            if (arg.len() > 1)
            {
                stdout = Some(CString::new(&arg[1..]).unwrap());
            } else if let Some(&file) = iter.next()
            {
                stdout = Some(CString::new(file).unwrap());
            }
        } else if (arg.starts_with('<'))
        {
            if (arg.len() > 1)
            {
                stdin = Some(CString::new(&arg[1..]).unwrap());
            } else if let Some(&file) = iter.next()
            {
                stdin = Some(CString::new(file).unwrap());
            }
        } else if arg.eq("|")
        {
            if !command.is_empty()
            {
                commands.push(Command
                {
                    command: command.clone(),
                    stdin: stdin.clone(),
                    stdout: stdout.clone(),
                });
                command.clear();
                stdin = None;
                stdout = None;
            }
        }
        else
        {
            command.push(CString::new(arg).unwrap());
        }
    }

    if (!command.is_empty())
    {
        commands.push(Command
        {
            command,
            stdin,
            stdout,
        });
    }

    commands
}

pub fn main() -> Result<(), Box<dyn Error>> {
    for line in io::stdin().lock().lines() {
        let line = line?;
        let mut commands = parse(&line);

        if commands.is_empty() {
            continue;
        }

        // Создание пайпов
        let mut previous_pipe: Option<(i32, i32)> = None;
        let total_command = commands.len();

        for (i, mut command) in commands.iter_mut().enumerate() {
            let (pipe_read, pipe_write) = if i < total_command - 1 {
                // Если не последняя команда, создаем новый пайп
                let (read_fd, write_fd) = pipe().expect("Failed to create pipe");
                (Some(read_fd), Some(write_fd))
            } else {
                (None, None)
            };

            match unsafe { fork() } {
                Ok(ForkResult::Parent { child }) => {
                    // Родительский процесс
                    waitpid(child, None)?;
                    if let Some((read_fd, write_fd)) = previous_pipe {
                        // Закрываем дескрипторы, которые больше не нужны
                        //close(read_fd).expect("Failed to close read end of the pipe");
                        close(write_fd).expect("Failed to close write end of the pipe");
                    }
                    previous_pipe = pipe_read.map(|pipe| (pipe_read.unwrap(), pipe_write.unwrap()));
                }
                Ok(ForkResult::Child) => {
                    // Дочерний процесс

                    // Перенаправление stdin, если указано
                    if i == 0 {
                        if let Some(ref input) = command.stdin {
                            let input_fd = open(input.as_c_str(), OFlag::O_RDONLY, Mode::empty())
                                .expect("Failed to open input file");
                            dup2(input_fd, STDIN_FILENO).expect("Failed to redirect stdin");
                        }
                    }

                    // Если есть предыдущий пайп, перенаправляем stdin из него
                    if let Some((read_fd, write_fd)) = previous_pipe {
                        dup2(read_fd, STDIN_FILENO).expect("Failed to redirect stdin from pipe");
                        // Read
                        let addition = read_from_fd(STDIN_FILENO)?;
                        let addition_str = String::from_utf8_lossy(&addition).to_string();
                        let addition_cstring = CString::new(addition_str)?;
                        command.command.insert(1, addition_cstring);
                        close(read_fd).expect("Failed to close read end of the pipe");
                    }

                    // Если есть текущий пайп, перенаправляем stdout в него
                    if let Some(write_fd) = pipe_write {
                        dup2(write_fd, STDOUT_FILENO).expect("Failed to redirect stdout to pipe");
                        //close(write_fd).expect("Failed to close write end of the pipe");
                    }

                    // Перенаправление stdout, если указано (для последней команды)
                    if i == total_command - 1 {
                        if let Some(ref output) = command.stdout {
                            let output_fd = open(output.as_c_str(), OFlag::O_WRONLY | OFlag::O_CREAT | OFlag::O_TRUNC, Mode::S_IRWXU)
                                .expect("Failed to open output file");
                            dup2(output_fd, STDOUT_FILENO).expect("Failed to redirect stdout");
                        }
                    }

                    // Выполняем команду
                    if let Err(e) = execvp(&command.command[0], &command.command) {
                        eprintln!("Command not found: {}", e);
                        std::process::exit(1);
                    }
                }
                Err(err) => {
                    eprintln!("Fork failed: {}", err);
                }
            }
        }
    }

    Ok(())
}