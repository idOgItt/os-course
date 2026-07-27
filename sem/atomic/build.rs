use std::{env, process::Command};

use anyhow::Result;


fn main() -> Result<()> {
    let out_dir = env::var("OUT_DIR")?;

    assert!(Command::new("as")
        .args(["-o", &(out_dir.clone() + "/atomic.o"), "src/impl.s"])
        .status()?
        .success());

    assert!(Command::new("ar")
        .args([
            "-crus",
            &(out_dir.clone() + "/libatomic.a"),
            &(out_dir.clone() + "/atomic.o")
        ])
        .status()?
        .success());

    println!("cargo:rustc-link-search=native={}", out_dir);
    println!("cargo:rustc-link-lib=static=atomic");

    Ok(())
}
