fn main() {
    // Build date for the version shown in the settings status bar.
    let date = std::process::Command::new("date").arg("+%Y-%m-%d").output().ok().and_then(|o| String::from_utf8(o.stdout).ok()).map(|s| s.trim().to_string()).unwrap_or_default();
    println!("cargo:rustc-env=BUILD_DATE={date}");
    tauri_build::build()
}
