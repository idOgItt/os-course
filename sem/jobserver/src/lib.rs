use std::{
    env::args,
    ffi::CString,
    fs::read_dir,
    os::unix::{fs::MetadataExt, prelude::RawFd},
    path::{Path, PathBuf},
    process::exit,
};
use std::error::Error;
use std::os::unix::fs::PermissionsExt;
use nix::{
    sys::resource::{getrlimit, Resource},
    sys::wait::waitpid,
    unistd::{close, execvp, fork, pipe, read, write, dup2, ForkResult},
};
use nix::libc::{STDIN_FILENO, STDOUT_FILENO};
use nix::sys::signal::{sigaction, SaFlags, SigAction, SigHandler, SigSet, Signal};

static mut PIPE_IN: RawFd = 0;
static mut PIPE_OUT: RawFd = 0;

fn semaphor_down() -> bool {
    let mut buffer = [0; 1];
    unsafe {
        match read(PIPE_IN, &mut buffer) {
            Ok(_) => true,
            Err(_) => false
        }
    }
}

fn semaphor_up() {
    let buffer = [1; 1];
    unsafe {
        write(PIPE_OUT, &buffer).expect("Failed to up semaphore");
    }
}

fn is_executable(metadata: &std::fs::Metadata) -> bool {
    metadata.permissions().mode() & 0o111 != 0
}

fn find_executables(dir: &Path) -> Vec<PathBuf> {
    let mut executables = Vec::new();
    if let Ok(entries) = read_dir(dir) {
        for entry in entries {
            if let Ok(entry) = entry {
                let path = entry.path();
                if path.is_dir() {
                    executables.extend(find_executables(&path));
                } else if let Ok(metadata) = entry.metadata() {
                    if is_executable(&metadata) {
                        executables.push(path);
                    }
                }
            }
        }
    }
    executables
}

extern "C" fn handle_sigchld(_signal: i32) {}

unsafe fn check_process_limit(active_processes: usize) -> bool {
    if let Ok((soft_limit, _)) = getrlimit(Resource::RLIMIT_NPROC) {
        return active_processes < soft_limit as usize;
    }
    true
}

fn wait_for_children(children_pids: &mut Vec<nix::unistd::Pid>) {
    for pid in children_pids.clone() {
        let _ = waitpid(None, None).expect("Error while waiting for child process");
        semaphor_up();
    }
    children_pids.clear();
}

unsafe fn run_command(executable: PathBuf) {
    let exec_str = CString::new(executable.to_str().unwrap()).expect("Failed to convert to CString");
    execvp(&exec_str, &[exec_str.clone()]).expect("Failed to execute process");
}

pub unsafe fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = args().collect();

    if args.len() != 2 {
        eprintln!("Usage: {} <parallelism>", args[0]);
        exit(1);
    }

    let limit_concurrency: usize = args[1].parse().expect("Fail concurrency value");

    let (pipe_in, pipe_out) = pipe().expect("Failed to create pipe");
    PIPE_IN = pipe_in;
    PIPE_OUT = pipe_out;

    let sig_action = SigAction::new(
        SigHandler::Handler(handle_sigchld),
        SaFlags::SA_RESTART,
        SigSet::empty(),
    );
    sigaction(Signal::SIGCHLD, &sig_action).expect("Failed to install SIGCHLD handler");

    for _ in 0..limit_concurrency {
        semaphor_up();
    }

    let executables = find_executables(Path::new("."));
    let mut children_pids = Vec::new();
    let mut previous_pipe: Option<(RawFd, RawFd)> = None;
    let mut active_processes = 0;

    for (i, executable) in executables.clone().into_iter().enumerate() {

        // if !semaphor_down() {
        //     wait_for_children(&mut children_pids);
        // }


        semaphor_down();

        // if (limit_concurrency == 1)
        // {
        //     semaphor_up();
        // }


        let (pipe_read, pipe_write) = if i < executables.clone().len() - 1 {
            let (read_fd, write_fd) = pipe().expect("Failed to create pipe");
            (Some(read_fd), Some(write_fd))
        } else {
            (None, None)
        };

        match fork() {
            Ok(ForkResult::Parent { child }) => {
                children_pids.push(child);
                active_processes += 1;


                // if i == 0
                // {
                //     semaphor_up();
                // }

                if let Some((read_fd, write_fd)) = previous_pipe {
                    close(read_fd).expect("Failed to close read end of the pipe");
                    close(write_fd).expect("Failed to close write end of the pipe");
                    //semaphor_up();
                }

                previous_pipe = pipe_read.map(|r| (r, pipe_write.unwrap()));

                if limit_concurrency == 1 ||(i % (limit_concurrency - 1) == 0)
                {
                    wait_for_children(&mut children_pids);
                }

                // semaphor_up(); // Освобождаем слот после форка
                // if !semaphor_down() {
                //     wait_for_children(&mut children_pids);
                // }
            }
            Ok(ForkResult::Child) => {
                if let Some((read_fd, _)) = previous_pipe {
                    dup2(read_fd, STDIN_FILENO).expect("Failed to redirect stdin from pipe");
                    close(read_fd).expect("Failed to close read end of the pipe in child");
                }

                if let Some(write_fd) = pipe_write {
                    dup2(write_fd, STDOUT_FILENO).expect("Failed to redirect stdout to pipe");
                    close(write_fd).expect("Failed to close write end of the pipe in child");
                }


                run_command(executable);
                exit(1);
            }
            Err(_) => {
                eprintln!("Fork fail");
                exit(1);
            }
        }
    }

    wait_for_children(&mut children_pids);

    println!("All processes completed");
    Ok(())
}
