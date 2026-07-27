use std::{
    env::args,
    ffi::CString,
    fs::read_dir,
    os::unix::{fs::MetadataExt, prelude::RawFd},
    path::{Path, PathBuf},
    process::exit,
    sync::atomic::{AtomicBool, Ordering},
    sync::Arc,
};
use std::error::Error;
use std::os::unix::fs::PermissionsExt;
use nix::{
    sys::resource::{getrlimit, Resource},
    sys::wait::waitpid,
    unistd::{close, execvp, fork, pipe, read, write, ForkResult},
};
use nix::libc::{dup, dup2, open, STDIN_FILENO, STDOUT_FILENO};
use nix::sys::signal::{sigaction, SaFlags, SigAction, SigHandler, SigSet, Signal};

static mut PIPE_IN: RawFd = 0;
static mut PIPE_OUT: RawFd = 0;

fn semaphor_down() {
    let mut buffer = [0; 1];
    unsafe {
        //dup2(PIPE_OUT, STDOUT_FILENO);
        // close(PIPE_OUT);
        // dup2(PIPE_IN, STDIN_FILENO);
        // close(PIPE_IN);
        //
        read(PIPE_IN, &mut buffer).expect("Failed to down semaphore");
    }
}

fn semaphor_up() {
    let  buffer = [1; 1];
    unsafe {
        // dup2(PIPE_IN, STDIN_FILENO);
        // close(PIPE_IN);
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

extern "C" fn handle_sigchld(_signal: i32) {
}

unsafe fn check_process_limit(active_processes: usize) -> bool {
    if let Ok((soft_limit, _)) = getrlimit(Resource::RLIMIT_NPROC) {
        return active_processes < soft_limit as usize;
    }
    true
}

unsafe fn run_command(executable: PathBuf) {
    let exec_str = CString::new(executable.to_str().unwrap()).expect("Failed to convert to CString");
    //dup2(PIPE_OUT, STDOUT_FILENO);
    close(PIPE_IN);
    close(PIPE_OUT);
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
    unsafe {
        PIPE_IN = pipe_in;
        PIPE_OUT = pipe_out;
    }

    let sig_action = SigAction::new(
        SigHandler::Handler(handle_sigchld),
        SaFlags::SA_RESTART,
        SigSet::empty(),
    );
    unsafe { sigaction(Signal::SIGCHLD, &sig_action).expect("Failed to install SIGCHLD handler") };

    for _ in 0..limit_concurrency {
        semaphor_up();
    }

    let executables = find_executables(Path::new("."));
    let mut children_pids = Vec::new();
    let mut active_processes = 0;

    for executable in executables {

        if !check_process_limit(active_processes) {
            eprintln!("Process limit exceeded. Waiting for child processes to finish.");
            break;
        }

        //dup2(PIPE_OUT, STDOUT_FILENO);
        //dup2(PIPE_IN, STDIN_FILENO);
        //close(PIPE_IN);
        //close(PIPE_OUT);

        semaphor_down();

        match unsafe { fork() } {
            Ok(ForkResult::Parent { child }) => unsafe {
                //close(PIPE_IN);
                // close(PIPE_OUT);
                //dup2(PIPE_IN, STDIN_FILENO);
                //close(PIPE_IN);
                //dup2(PIPE_OUT, STDOUT_FILENO);
                // close(PIPE_OUT);
                children_pids.push(child);
                active_processes += 1;
            }
            Ok(ForkResult::Child) => unsafe {

                close(PIPE_IN);
                //close(PIPE_OUT);
                run_command(executable);

                eprintln!("Fail to excute in child");
                exit(1);
            }
            Err(_) => {
                eprintln!("Fork fail");
                exit(1);
            }
        }
    }

    for pid in children_pids {
        let _ = waitpid(pid, None).expect("Error while waiting for child process");
        // dup2(PIPE_IN, STDIN_FILENO);
        semaphor_up();
        active_processes -= 1;
    }

    println!("All processes completed");
    Ok(())
}
