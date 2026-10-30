//! Tauri build hook: compiles the config, capabilities, and icons into the
//! binary at build time.

fn main() {
    tauri_build::build();
}
