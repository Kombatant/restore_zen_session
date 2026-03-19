use std::{
    env, fs,
    path::{Path, PathBuf},
};

fn main() {
    let qt_include_path = env::var("DEP_QT_INCLUDE_PATH").expect("DEP_QT_INCLUDE_PATH not set");

    let mut config = cpp_build::Config::new();
    for flag in env::var("DEP_QT_COMPILE_FLAGS")
        .expect("DEP_QT_COMPILE_FLAGS not set")
        .split_terminator(';')
    {
        config.flag(flag);
    }

    config.include(qt_include_path).build("src/gui.rs");

    copy_google_oauth_config();
}

fn copy_google_oauth_config() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set"));
    let source_path = manifest_dir.join("google.json");
    println!("cargo:rerun-if-changed={}", source_path.display());

    if !source_path.is_file() {
        println!(
            "cargo:warning=google.json was not found at {}. Google Drive sign-in will not work until you add it.",
            source_path.display()
        );
        return;
    }

    let profile = env::var("PROFILE").expect("PROFILE not set");
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));
    let Some(target_profile_dir) = find_profile_dir(&out_dir, &profile) else {
        println!(
            "cargo:warning=Could not determine the Cargo output directory for profile {}. google.json was not copied.",
            profile
        );
        return;
    };

    let destination_path = target_profile_dir.join("google.json");
    if let Err(error) = fs::copy(&source_path, &destination_path) {
        panic!(
            "failed to copy {} to {}: {error}",
            source_path.display(),
            destination_path.display()
        );
    }
}

fn find_profile_dir(out_dir: &Path, profile: &str) -> Option<PathBuf> {
    out_dir
        .ancestors()
        .find(|path| path.file_name().is_some_and(|name| name == profile))
        .map(Path::to_path_buf)
}
