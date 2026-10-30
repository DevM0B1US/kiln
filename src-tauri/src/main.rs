//! Kiln desktop entry: window, or a headless pipeline harness.
//!
//! `kiln`            — the desktop app
//! `kiln cut`        — remove backgrounds, headless
//! `kiln merge`      — template merge, headless

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(code) = kiln_lib::cli::dispatch(&args) {
        let raw = if code == std::process::ExitCode::SUCCESS {
            0
        } else {
            1
        };
        std::process::exit(raw);
    }
    kiln_lib::run();
}
