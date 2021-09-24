use std::process;

fn main() {
    println!("cargo:rerun-if-changed=dashboard/");

    assert!(process::Command::new("npm")
        .current_dir("dashboard")
        .arg("run")
        .arg("build")
        .status()
        .expect("Command failed to run.")
        .success());
}
