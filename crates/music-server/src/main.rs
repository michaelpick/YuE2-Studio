//! Standalone entry point for the studio service.
//!
//! The desktop application embeds `music_server::serve` directly; this binary
//! exists for development and for headless use.
//!
//! A headless studio has no window, so nothing hands the interface over to a
//! browser: the desktop shell does that with Tauri's asset resolver. This
//! binary registers one itself, reading the built frontend from a folder
//! (`YUE_INTERFACE_DIR`, or `app/dist` beside the executable), so `/` in a
//! browser is the studio rather than a 404.

use std::path::{Path, PathBuf};

/// Where the built frontend lives, if this build has one.
fn interface_directory() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("YUE_INTERFACE_DIR") {
        let dir = PathBuf::from(dir);
        if dir.is_dir() {
            return Some(dir);
        }
        eprintln!("[studio] YUE_INTERFACE_DIR {} is not a folder", dir.display());
    }
    // target/release/music-server sits two levels below the repository root,
    // which is where the frontend is built.
    let beside = std::env::current_exe().ok()?.parent()?.join("../../app/dist");
    let beside = beside.canonicalize().ok()?;
    beside.is_dir().then_some(beside)
}

fn media_type(path: &Path) -> String {
    let extension = path.extension().and_then(|value| value.to_str()).unwrap_or_default();
    let media_type = match extension {
        "html" => "text/html; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "wasm" => "application/wasm",
        "mp3" => "audio/mpeg",
        "mp4" => "video/mp4",
        "py" => "text/x-python; charset=utf-8",
        _ => "application/octet-stream",
    };
    media_type.to_string()
}

fn main() -> anyhow::Result<()> {
    if !music_engine::process_group::bind_children_to_this_process() {
        eprintln!("this process could not create its own job object; the engine is stopped by the supervisor only");
    }
    music_server::apply_saved_gpu();
    // the port is decided before the async workers start: deciding it can wait on netsh
    music_engine::yue_server::engine_port();
    match interface_directory() {
        Some(root) => {
            let served_from = root.clone();
            music_server::set_asset_source(Box::new(move |path: &str| {
                // A request path never escapes the folder it is served from.
                if path.split('/').any(|part| part == "..") {
                    return None;
                }
                let file = root.join(path);
                let bytes = std::fs::read(&file).ok()?;
                Some((bytes, media_type(&file)))
            }));
            eprintln!("[studio] the interface is served from {}", served_from.display());
        }
        None => eprintln!("[studio] no interface folder found: a browser gets 404 at /"),
    }
    if let Ok(version) = std::env::var("YUE_STUDIO_VERSION") {
        music_server::set_studio_version(version);
    }
    tokio::runtime::Builder::new_multi_thread().enable_all().build()?.block_on(music_server::serve())
}
