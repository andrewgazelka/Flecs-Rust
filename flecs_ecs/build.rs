//! Re-exports flecs's C symbols from this crate's dylib.
//!
//! This lives here rather than in `flecs_ecs_sys`, where flecs's C actually is, because a
//! build script's `rustc-link-arg` applies to its own crate's artifacts and nothing else.
//! `flecs_ecs_sys` is an rlib absorbed into this dylib rather than linked itself, so the
//! same code placed there does nothing at all -- measured, zero exported `ecs_*`, and no
//! warning from cargo or the linker.
//!
//! ## Why any of this is needed
//!
//! rustc links a `dylib` with its own anonymous version script ending in `local: *`, which
//! demotes every symbol it did not generate. flecs's C symbols reach the object with
//! `DEFAULT` visibility and `LOCAL` binding, which is to say present and unreachable:
//!
//! ```text
//! $ readelf -sW libflecs_ecs.so | grep -w ecs_init
//!   5905: ... FUNC    LOCAL  DEFAULT   14 ecs_init
//! ```
//!
//! A consumer that loads a second module at runtime then cannot resolve `ecs_*` here and
//! links its own static copy of flecs instead, which is two `ecs_os_api` globals and two
//! component-index pools in one process.
//!
//! Neither `-Wl,--export-dynamic` nor `-Wl,--export-dynamic-symbol=ecs_*` reverses a
//! version-script demotion; both were measured leaving the exported count at zero. What
//! works is a second version script: ld merges them, and an explicit pattern beats a `*`
//! wildcard, so naming these globs promotes exactly them while leaving rustc's own export
//! list intact. Measured after: 666 exported `ecs_*`, `ecs_init` `GLOBAL`.

/// Spelled without the leading underscore Mach-O adds.
const FLECS_EXPORTS: [&str; 4] = ["ecs_*", "flecs_*", "Ecs*", "FLECS_*"];

fn main() {
    println!("cargo::rerun-if-changed=build.rs");

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if matches!(target_os.as_str(), "macos" | "ios") {
        // ld64 unions -exported_symbol with the export list rustc generates, so no second
        // script is needed and none is available.
        for pattern in FLECS_EXPORTS {
            println!("cargo::rustc-link-arg=-Wl,-exported_symbol,_{pattern}");
        }
        return;
    }

    let out_dir = std::env::var("OUT_DIR").expect("cargo always sets OUT_DIR");
    let script = std::path::Path::new(&out_dir).join("flecs-exports.map");

    let mut text = String::from("{\n  global:\n");
    for pattern in FLECS_EXPORTS {
        text.push_str("    ");
        text.push_str(pattern);
        text.push_str(";\n");
    }
    // Deliberately no `local:` clause. This script adds to rustc's export list; a
    // `local: *` here would hide every Rust symbol a consumer resolves through.
    text.push_str("};\n");

    std::fs::write(&script, text).expect("failed to write the flecs version script");
    println!(
        "cargo::rustc-link-arg=-Wl,--version-script={}",
        script.display()
    );
}
