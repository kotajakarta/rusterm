//! Image paste for terminal CLIs (Claude Code, etc.): the browser uploads the clipboard image,
//! it is stored on the managed server, and the terminal receives the file path.

use rand::RngCore;
use serde_json::json;

/// Folder (relative to the remote user's home) that holds pasted images.
pub const PASTE_DIR: &str = ".cache/rusterm-paste";
/// Pasted images older than this are pruned on the next paste.
pub const PASTE_KEEP_SECONDS: u64 = 24 * 60 * 60;
/// Largest accepted image.
pub const PASTE_MAX_BYTES: usize = 10 * 1024 * 1024;

/// Detects the image format from its magic bytes; SVG and anything else is rejected.
pub fn sniff_image_type(data: &[u8]) -> Option<&'static str> {
    if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("png")
    } else if data.starts_with(b"\xff\xd8\xff") {
        Some("jpg")
    } else if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") {
        Some("gif")
    } else if data.len() >= 12 && &data[0..4] == b"RIFF" && &data[8..12] == b"WEBP" {
        Some("webp")
    } else {
        None
    }
}

/// `paste-YYYYMMDD-HHMMSS-xxxxxx.<ext>`; the random suffix keeps same-second pastes apart.
pub fn paste_image_name(ext: &str, at: chrono::NaiveDateTime) -> String {
    let mut suffix = [0u8; 3];
    rand::thread_rng().fill_bytes(&mut suffix);
    let hex: String = suffix.iter().map(|b| format!("{:02x}", b)).collect();
    format!("paste-{}-{}.{}", at.format("%Y%m%d-%H%M%S"), hex, ext)
}

/// Stdin for the files agent `paste_image` op: one JSON header line, then the image as
/// length-prefixed chunks (4-byte big endian), terminated by a zero-length chunk.
pub fn build_paste_request(name: &str, data: &[u8]) -> Vec<u8> {
    let header = json!({
        "op": "paste_image",
        "dir": PASTE_DIR,
        "name": name,
        "keep_seconds": PASTE_KEEP_SECONDS,
    });
    let mut out = header.to_string().into_bytes();
    out.push(b'\n');
    const CHUNK_SIZE: usize = 64 * 1024;
    for chunk in data.chunks(CHUNK_SIZE) {
        out.extend_from_slice(&(chunk.len() as u32).to_be_bytes());
        out.extend_from_slice(chunk);
    }
    out.extend_from_slice(&0u32.to_be_bytes());
    out
}
