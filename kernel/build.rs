use std::{env, error::Error, fs, process::Command, str};


fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed=../Cargo.lock");
    println!("cargo:rerun-if-changed=../fs.img");
    println!("cargo:rerun-if-changed=../ku/Cargo.toml");
    println!("cargo:rerun-if-changed=../ku/src");

    let binaries = vec![
        "lib",
        "cow_fork",
        "eager_fork",
        "exit",
        "log_value",
        "loop",
        "memory_allocator",
        "page_fault",
        "sched_yield",
        "trap_handler",
    ];

    for bin in binaries {
        println!("cargo:rerun-if-changed=../user/{}/Cargo.toml", bin);
        println!("cargo:rerun-if-changed=../user/{}/src", bin);

        let terminate = || panic!("failed to build user binary {}", bin);

        let output = Command::new("cargo")
            .args(["build", "--profile", "user"])
            .current_dir(format!("../user/{}", bin))
            .env_clear()
            .env("PATH", env::var("PATH")?)
            .output()
            .unwrap_or_else(|_| terminate());

        if !output.status.success() {
            println!(
                "cargo:warning=\n{}",
                str::from_utf8(output.stderr.as_slice()).unwrap_or("<some invalid UTF-8>"),
            );
            terminate();
        }
    }

    if make_fs_image("../fs.img", 32 << 20).is_err() {
        let message = "failed to create a disk image for the file system";
        println!("cargo:warning=\n{}", message);
        panic!("{}", message);
    }

    Ok(())
}


fn make_fs_image(path: &str, size: usize) -> Result<(), std::io::Error> {
    fs::remove_file(path).unwrap_or(());
    fs::write(path, vec![0_u8; size])
}
