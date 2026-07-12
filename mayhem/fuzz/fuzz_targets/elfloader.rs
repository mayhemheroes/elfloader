#![no_main]

use libfuzzer_sys::fuzz_target;

// Upstream is a binary-only crate (no lib target), so include its source directly:
// this compiles write_file() — the whole ELF->FELF parse/convert path — into the
// fuzz binary with full sanitizer + coverage instrumentation, without touching
// any upstream file (purely additive overlay).
#[path = "../../../src/main.rs"]
#[allow(dead_code)]
mod elfloader;

fuzz_target!(|data: &[u8]| {
    // write_file() takes a filesystem path (it opens the ELF itself), so stage the
    // fuzz input in a per-process scratch file under /tmp (the image dir is
    // mounted read-only during coverage collection).
    let path = std::env::temp_dir().join(format!("elfloader_fuzz_{}", std::process::id()));
    if std::fs::write(&path, data).is_err() {
        return;
    }

    // Drive the same code path as the CLI, across its three output modes.
    let mut felf = Vec::new();
    let _ = elfloader::write_file(&path, None, &mut felf, false, false);

    let mut felf_perms = Vec::new();
    let _ = elfloader::write_file(&path, Some(0), &mut felf_perms, false, true);

    let mut raw = Vec::new();
    let _ = elfloader::write_file(&path, None, &mut raw, true, false);
});
