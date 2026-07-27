#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

use anyhow::{bail, Context, Result};
use glob::glob;
use serde::Deserialize;
use structopt::StructOpt;

use std::{env, fs, path::PathBuf, process::Command};

////////////////////////////////////////////////////////////////////////////////

const CONFIG_NAME: &str = ".deadlines.yml";
const REPORT_URL: &str = "https://os.manytask.org/api/report";
const SUDO_PRESERVE_ENVS: &[&str] = &["PATH", "HOME", "CARGO_HOME", "RUSTUP_HOME"];
const TESTER_TOKEN_ENV: &str = "TESTER_TOKEN";

#[derive(Deserialize, Clone)]
struct Task {
    task: String,
    allow_list: Vec<String>,
    check: Vec<String>,
}

#[derive(Deserialize, Clone)]
struct Group {
    tasks: Vec<Task>,
}
#[derive(Deserialize, Clone)]
struct Config {
    groups: Vec<Group>,
}

impl Config {
    fn find_task(&self, task_name: &str) -> Option<&Task> {
        for group in &self.groups {
            for task in &group.tasks {
                if task.task == task_name {
                    return Some(task);
                }
            }
        }

        None
    }
}

fn read_config() -> Result<Config> {
    let config = fs::read_to_string(CONFIG_NAME)?;
    let groups = serde_yaml::from_str(config.as_str())?;
    Ok(Config { groups })
}

#[derive(StructOpt, Debug)]
#[structopt()]
struct Opts {
    /// Path to the student repository.
    #[structopt(short = "s", long = "student-repo")]
    student_repo: PathBuf,
    /// Path to the original repo.
    #[structopt(short = "o", long = "original-repo")]
    original_repo: PathBuf,
    /// CI branch name (e.g. "submit/add")
    #[structopt(short = "b", long = "ci-branch-name")]
    ci_branch_name: String,
    /// Gitlab User ID
    #[structopt(short = "u", long = "user-id")]
    user_id: String,

    #[structopt(long = "sandbox")]
    sandbox: bool,
}

////////////////////////////////////////////////////////////////////////////////

fn parse_task_name(branch: &str) -> Result<&str> {
    const PREFIX: &str = "submit/";
    if !branch.starts_with(PREFIX) {
        bail!("branch '{}' does not start with '{}'", branch, PREFIX);
    }

    let task = &branch[PREFIX.len()..];
    if task.contains('/') {
        bail!("task '{}' contains '/'", task);
    }

    Ok(task)
}

fn check_task(opts: &Opts, task: &Task) -> Result<()> {
    for line in &task.check {
        let mut cmd = if opts.sandbox {
            Command::new("sudo")
        } else {
            Command::new("sh")
        };

        if opts.sandbox {
            cmd.args(SUDO_PRESERVE_ENVS.iter().map(|e| format!("--preserve-env={}", e)))
                .args(&["-u", "nobody", "sh"]);
        }

        cmd.args(&["-cex"]).arg(&line).current_dir(&opts.original_repo);

        println!(
            "> {} {}",
            cmd.get_program().to_str().unwrap(),
            cmd.get_args()
                .map(|s| format!("\"{}\"", s.to_str().unwrap()))
                .collect::<Vec<_>>()
                .join(" ")
        );

        let status = cmd.spawn()?.wait()?;

        if !status.success() {
            bail!("'{}' command terminated with error", line);
        }
    }

    Ok(())
}

fn submit_report(user_id: &str, task_name: &str, tester_token: &str) -> Result<()> {
    let form = reqwest::blocking::multipart::Form::new()
        .text("user_id", user_id.to_string())
        .text("task", task_name.to_string())
        .text("token", tester_token.to_string());
    reqwest::blocking::Client::new()
        .post(REPORT_URL)
        .multipart(form)
        .send()?
        .error_for_status()?;
    Ok(())
}

fn do_main(opts: Opts) -> Result<()> {
    let task_name = parse_task_name(&opts.ci_branch_name).context("failed to parse task name")?;
    let config = read_config()?;

    let task = config.find_task(task_name);
    match task {
        Some(task) => {
            let orig_path = &opts.original_repo;
            let stud_path = &opts.student_repo;

            for pattern in &task.allow_list {
                let path = orig_path.join(pattern.as_str());

                for entry in glob(path.to_str().unwrap())? {
                    let dst = entry?;

                    let relative = if orig_path.as_os_str() == "." {
                        dst.as_path()
                    } else {
                        dst.strip_prefix(&orig_path).with_context(|| {
                            format!("orig_path: {}, dst: {}", orig_path.display(), dst.display())
                        })?
                    };

                    let src = stud_path.join(relative);

                    println!("* copy {}", relative.display());
                    if orig_path != stud_path {
                        fs::copy(&src, &dst).with_context(|| {
                            format!("failed to copy {} to {}", src.display(), dst.display())
                        })?;
                    }
                }
            }

            check_task(&opts, task).context("task check failed")?;
            let tester_token = env::var(TESTER_TOKEN_ENV).with_context(|| {
                format!("envirnnment variable '{}' is not set", TESTER_TOKEN_ENV)
            })?;
            submit_report(&opts.user_id, &task_name, &tester_token)
                .context("failed to submit report")
        },

        None => {
            bail!("task {} is not defined in {}", task_name, CONFIG_NAME)
        },
    }
}

fn main() {
    let args = Opts::from_args();

    if let Err(err) = do_main(args) {
        eprintln!("Error: {:#}", err);
        std::process::exit(1);
    }
}
