//! elfloader-kat — behavioral known-answer test for elfloader's write_file().
//!
//! Upstream ships no test suite, so this oracle asserts the EXACT output bytes of
//! the ELF -> FELF conversion for a crafted 64-bit little-endian ELF with one
//! PT_LOAD (R+X) segment, across all three output modes (FELF0001, FELF0002 with
//! permissions, and --binary raw). The expected byte vectors are hardcoded golden
//! answers derived from the FELF format spec, NOT recomputed from the code under
//! test — a patch that breaks the conversion (or no-ops it) fails these checks.
//!
//! Prints one PASS/FAIL line per check and a final "CHECKS <n> FAILURES <m>"
//! summary line (parsed by mayhem/test.sh); exits non-zero iff any check failed.

// Upstream is a binary-only crate (no lib target): include its source directly so
// we call the real write_file(), without modifying any upstream file.
#[path = "../../../src/main.rs"]
#[allow(dead_code)]
mod elfloader;

/// Crafted 64-bit LE ELF: 64-byte header, one PT_LOAD program header at offset 64
/// (flags R+X, vaddr 0x1000, filesz=memsz=4), 4 segment bytes DE AD BE EF at 120.
fn test_elf() -> Vec<u8> {
    let mut e = Vec::new();
    // e_ident
    e.extend_from_slice(b"\x7fELF"); // magic
    e.push(2); // EI_CLASS: 64-bit
    e.push(1); // EI_DATA: little-endian
    e.push(1); // EI_VERSION
    e.push(0); // EI_OSABI
    e.push(0); // EI_ABIVERSION
    e.extend_from_slice(&[0u8; 7]); // padding
    e.extend_from_slice(&2u16.to_le_bytes()); // e_type: EXEC
    e.extend_from_slice(&0x3eu16.to_le_bytes()); // e_machine: x86-64
    e.extend_from_slice(&1u32.to_le_bytes()); // e_version
    e.extend_from_slice(&0x1000u64.to_le_bytes()); // e_entry
    e.extend_from_slice(&64u64.to_le_bytes()); // e_phoff
    e.extend_from_slice(&0u64.to_le_bytes()); // e_shoff
    e.extend_from_slice(&0u32.to_le_bytes()); // e_flags
    e.extend_from_slice(&64u16.to_le_bytes()); // e_ehsize
    e.extend_from_slice(&56u16.to_le_bytes()); // e_phentsize
    e.extend_from_slice(&1u16.to_le_bytes()); // e_phnum
    e.extend_from_slice(&[0u8; 6]); // e_shentsize/e_shnum/e_shstrndx (unread)
    assert_eq!(e.len(), 64);
    // program header (ELF64, 56 bytes)
    e.extend_from_slice(&1u32.to_le_bytes()); // p_type: PT_LOAD
    e.extend_from_slice(&5u32.to_le_bytes()); // p_flags: PF_R | PF_X
    e.extend_from_slice(&120u64.to_le_bytes()); // p_offset
    e.extend_from_slice(&0x1000u64.to_le_bytes()); // p_vaddr
    e.extend_from_slice(&0u64.to_le_bytes()); // p_paddr
    e.extend_from_slice(&4u64.to_le_bytes()); // p_filesz
    e.extend_from_slice(&4u64.to_le_bytes()); // p_memsz
    e.extend_from_slice(&0u64.to_le_bytes()); // p_align
    assert_eq!(e.len(), 120);
    // segment bytes
    e.extend_from_slice(&[0xde, 0xad, 0xbe, 0xef]);
    e
}

fn check(name: &str, got: &[u8], want: &[u8]) -> bool {
    if got == want {
        println!("PASS {}", name);
        true
    } else {
        println!("FAIL {}: got {} bytes {:02x?}, want {} bytes {:02x?}",
                 name, got.len(), got, want.len(), want);
        false
    }
}

fn main() {
    let input = test_elf();
    let path = std::env::temp_dir().join(format!("elfloader_kat_{}", std::process::id()));
    std::fs::write(&path, &input).expect("write test ELF");

    let mut failures = 0usize;
    let mut checks = 0usize;

    // Golden answer 1: default FELF0001 = magic + entry(le64) + base(le64) + segment bytes.
    let mut want_felf1: Vec<u8> = Vec::new();
    want_felf1.extend_from_slice(b"FELF0001");
    want_felf1.extend_from_slice(&0x1000u64.to_le_bytes()); // entry
    want_felf1.extend_from_slice(&0x1000u64.to_le_bytes()); // base (lowest PT_LOAD vaddr)
    want_felf1.extend_from_slice(&[0xde, 0xad, 0xbe, 0xef]);
    let mut got = Vec::new();
    match elfloader::write_file(&path, None, &mut got, false, false) {
        Ok(()) => { checks += 1; if !check("felf0001", &got, &want_felf1) { failures += 1; } }
        Err(e) => { checks += 1; failures += 1; println!("FAIL felf0001: write_file errored: {:?}", e); }
    }

    // Golden answer 2: --perms FELF0002 = magic + entry + base + segment bytes + perm bytes
    // (one byte per loaded byte; R|X = 4|1 = 5).
    let mut want_felf2: Vec<u8> = Vec::new();
    want_felf2.extend_from_slice(b"FELF0002");
    want_felf2.extend_from_slice(&0x1000u64.to_le_bytes());
    want_felf2.extend_from_slice(&0x1000u64.to_le_bytes());
    want_felf2.extend_from_slice(&[0xde, 0xad, 0xbe, 0xef]);
    want_felf2.extend_from_slice(&[5, 5, 5, 5]);
    let mut got = Vec::new();
    match elfloader::write_file(&path, None, &mut got, false, true) {
        Ok(()) => { checks += 1; if !check("felf0002-perms", &got, &want_felf2) { failures += 1; } }
        Err(e) => { checks += 1; failures += 1; println!("FAIL felf0002-perms: write_file errored: {:?}", e); }
    }

    // Golden answer 3: --binary raw image = just the loaded segment bytes.
    let want_raw: Vec<u8> = vec![0xde, 0xad, 0xbe, 0xef];
    let mut got = Vec::new();
    match elfloader::write_file(&path, None, &mut got, true, false) {
        Ok(()) => { checks += 1; if !check("binary-raw", &got, &want_raw) { failures += 1; } }
        Err(e) => { checks += 1; failures += 1; println!("FAIL binary-raw: write_file errored: {:?}", e); }
    }

    // Negative known-answer: a non-ELF input must be REJECTED (MissingMagic), not converted.
    let bad = std::env::temp_dir().join(format!("elfloader_kat_bad_{}", std::process::id()));
    std::fs::write(&bad, b"not an elf at all").expect("write bad input");
    let mut got = Vec::new();
    checks += 1;
    match elfloader::write_file(&bad, None, &mut got, false, false) {
        Err(elfloader::Error::MissingMagic) if got.is_empty() => println!("PASS reject-non-elf"),
        other => { failures += 1; println!("FAIL reject-non-elf: expected Err(MissingMagic) w/ empty output, got {:?} ({} bytes)", other.err(), got.len()); }
    }

    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(&bad);

    println!("CHECKS {} FAILURES {}", checks, failures);
    if failures > 0 {
        std::process::exit(1);
    }
}
