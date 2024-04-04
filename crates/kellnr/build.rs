use std::{fs, path::Path, process::Command};

static UI_DIR: &str = "../../ui";
static UI_DIR_SRC: &str = "../../ui/src";
static UI_DIST_DIR: &str = "../../ui/dist";
static STATIC_DIR: &str = "../../static";
static INDEX_HTML: &str = "../../ui/index.html";

#[cfg(windows)]
static NPM_CMD: &str = "npm.cmd";
#[cfg(not(windows))]
static NPM_CMD: &str = "npm";

fn main() {
    println!("Build Kellnr - build.rs!");

    println!("cargo:rerun-if-changed={}", UI_DIR_SRC);
    println!("cargo:rerun-if-changed={}", INDEX_HTML);

    install_ui_deps();
    build_ui();
    copy_dir_all(UI_DIST_DIR, STATIC_DIR);
}

fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) {
    fs::create_dir_all(&dst).expect("failed to create dir all");
    for entry in fs::read_dir(src).expect("failed to read dir") {
        let entry = entry.expect("failed to get entry in dir");
        let ty = entry.file_type().expect("failed to get file type");
        if ty.is_dir() {
            copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()));
        } else {
            fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))
                .expect("failed to copy to destination");
        }
    }
}

fn install_ui_deps() {
    let ui_path = Path::new(UI_DIR)
        .canonicalize()
        .expect("failed to canonicalize ui path");
    if !ui_path.join("node_modules").exists() {
        println!("Installing node dependencies...");
        Command::new(NPM_CMD)
            .args(["install"])
            .current_dir(ui_path)
            .status()
            .expect("failed to install node modules");
    }
}

fn build_ui() {
    let ui_path = Path::new(UI_DIR)
        .canonicalize()
        .expect("failed to canonicalize ui path");
    println!("Building UI...");
    Command::new(NPM_CMD)
        .args(["run", "build"])
        .current_dir(ui_path)
        .status()
        .expect("failed to build ui");
}
