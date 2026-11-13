//! Builds libopus from the vendored source (shared/vendor/opus),
//! as a plain float build with no SIMD dispatch.

use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-env-changed=OWLMIC_SKIP_NATIVE");
    let target_windows = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows");
    let host_windows = std::env::var("HOST").is_ok_and(|h| h.contains("windows"));
    // A Windows build checked from another system has no MSVC to compile C with; checking needs
    // no object code, and the real build runs on Windows.
    if std::env::var_os("OWLMIC_SKIP_NATIVE").is_some() || (target_windows && !host_windows) {
        return;
    }
    let opus = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../shared/vendor/opus");
    assert!(
        opus.join("include/opus.h").exists(),
        "libopus source missing at {}: run `git submodule update --init`",
        opus.display()
    );
    let mut build = cc::Build::new();
    for (list, var) in [
        ("celt_sources.mk", "CELT_SOURCES"),
        ("silk_sources.mk", "SILK_SOURCES"),
        ("silk_sources.mk", "SILK_SOURCES_FLOAT"),
        ("opus_sources.mk", "OPUS_SOURCES"),
        ("opus_sources.mk", "OPUS_SOURCES_FLOAT"),
    ] {
        for file in sources(&opus.join(list), var) {
            build.file(opus.join(file));
        }
    }
    for dir in ["include", "celt", "silk", "silk/float", "src"] {
        build.include(opus.join(dir));
    }
    build
        .define("OPUS_BUILD", None)
        .define("USE_ALLOCA", None)
        .define("NDEBUG", None)
        .warnings(false);
    if !target_windows {
        build.define("HAVE_LRINTF", None);
    }
    build.compile("opus");
    println!(
        "cargo:rerun-if-changed={}",
        opus.join("include/opus.h").display()
    );
}

/// The files a Makefile variable lists, such as `CELT_SOURCES = celt/bands.c \ ...`.
fn sources(mk: &Path, var: &str) -> Vec<String> {
    let text = std::fs::read_to_string(mk).unwrap_or_else(|e| panic!("{}: {e}", mk.display()));
    let mut out = Vec::new();
    let mut inside = false;
    for line in text.lines() {
        let line = line.trim();
        if !inside {
            if let Some(rest) = line
                .strip_prefix(var)
                .and_then(|r| r.trim_start().strip_prefix('='))
            {
                out.extend(
                    rest.split_whitespace()
                        .filter(|w| *w != "\\")
                        .map(str::to_owned),
                );
                inside = line.ends_with('\\');
            }
            continue;
        }
        out.extend(
            line.split_whitespace()
                .filter(|w| *w != "\\")
                .map(str::to_owned),
        );
        inside = line.ends_with('\\');
    }
    out
}
