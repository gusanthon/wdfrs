//! `cargo wdffi` — build FFI artifacts for any crate that uses `export_wdf!`.
//!
//! This cargo subcommand automates the two-step process of exposing a wdfrs
//! circuit as a C-compatible library. Run it from the root of your own crate
//! (the one that calls `export_wdf!`), not from the wdfrs workspace itself.
//!
//! 1. **Build** — compiles your crate as a `staticlib` in release mode
//!    (and `--no-default-features` to avoid pulling in plotting/std overhead).
//! 2. **Header generation** — macro-expands your crate with `cargo expand` and
//!    feeds the result to `cbindgen` to produce a C header.
//!
//! # Installation
//!
//! Clone the wdfrs repository, then install from the workspace:
//!
//! ```sh
//! # from a clone of this repository:
//! cargo install --path cargo-wdffi
//! ```
//!
//! Requires [`cargo-expand`](https://github.com/dtolnay/cargo-expand) and
//! [`cbindgen`](https://github.com/mozilla/cbindgen) on `$PATH`.
//!
//! # Usage
//!
//! Run from the root of your crate (the one using `export_wdf!`):
//!
//! Single header (all exports):
//! ```sh
//! cargo wdffi output/myfilter.h
//! ```
//!
//! One header per `export_wdf!` prefix (output to directory):
//! ```sh
//! cargo wdffi output/headers/
//! ```
//!
//! If a `cbindgen.toml` is present in your crate root it will be passed to
//! cbindgen automatically.

use std::path::{Path, PathBuf};
use std::process::Command;

fn cwd() -> PathBuf {
    std::env::current_dir().expect("failed to get current directory")
}

fn main() {
    // When invoked as `cargo wdffi <arg>`, cargo passes "wdffi" as the first
    // argument. Skip it so both `cargo-wdffi <arg>` and `cargo wdffi <arg>`
    // work identically.
    let mut args = std::env::args().skip(1).peekable();
    if args.peek().map(String::as_str) == Some("wdffi") {
        args.next();
    }

    let output = args.next().unwrap_or_else(|| {
        help();
        std::process::exit(1);
    });

    let output_path = Path::new(&output);
    let is_dir = output.ends_with('/')
        || output.ends_with(std::path::MAIN_SEPARATOR)
        || output_path.is_dir();
    let is_header = output.ends_with(".h") || output.ends_with(".hpp");

    if is_dir {
        build_ffi_multi(&output);
    } else if is_header {
        build_ffi(&output);
    } else {
        eprintln!(
            "error: ambiguous output path {output:?}\n\nhint: use a .h extension for a single header, \
             or a trailing slash for per-prefix headers:\n\n  cargo wdffi output.h\n  cargo wdffi headers/"
        );
        std::process::exit(1);
    }
}

fn help() {
    eprintln!(
        "usage:
  cargo wdffi <output.h>          — single header (all exports)
  cargo wdffi <output-dir>/       — one header per export_wdf! prefix

builds the release staticlib and generates C header(s) via cbindgen.

requires: cargo-expand, cbindgen
"
    );
}

/// Run a [`Command`], panicking if it fails to spawn or exits non-zero.
fn run(cmd: &mut Command) {
    let status = cmd
        .status()
        .unwrap_or_else(|e| panic!("failed to run command: {e}"));
    assert!(status.success(), "command exited with status {status}");
}

/// Read the `[package] name` from `Cargo.toml` in `root`, converting hyphens
/// to underscores to match the `.a` filename Cargo produces.
fn read_crate_name(root: &Path) -> Option<String> {
    let cargo_toml = std::fs::read_to_string(root.join("Cargo.toml")).ok()?;
    for line in cargo_toml.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("name") {
            let rest = rest.trim();
            if let Some(rest) = rest.strip_prefix('=') {
                let name = rest.trim().trim_matches('"').trim_matches('\'');
                if !name.is_empty() {
                    return Some(name.replace('-', "_"));
                }
            }
        }
    }
    None
}

/// Shared first two steps: `cargo build` → strip `.a` → `cargo expand`.
///
/// Returns `(lib_name, path_to_expanded_rs)`.
fn build_and_expand(root: &Path) -> (String, PathBuf) {
    let expanded = std::env::temp_dir().join("wdfrs_expanded.rs");
    let build_dir = std::env::temp_dir().join("wdfrs_build");

    println!("building release staticlib...");
    run(Command::new("cargo")
        .args(["build", "--release", "--no-default-features"])
        .current_dir(root));

    let lib_name = read_crate_name(root).unwrap_or_else(|| "<name>".to_string());
    let lib_path = root.join(format!("target/release/lib{lib_name}.a"));

    if lib_path.exists() {
        println!("stripping debug info from lib{lib_name}.a...");
        // `strip --strip-debug` removes DWARF sections that the profile
        // `strip = true` flag doesn't reach for staticlib archives.
        let _ = Command::new("strip")
            .args(["--strip-debug", lib_path.to_str().unwrap()])
            .status();
    }

    println!("expanding macros...");
    let expand_out = std::fs::File::create(&expanded).expect("failed to create temp file");
    run(Command::new("cargo")
        .args([
            "expand",
            "--lib",
            "--all-features",
            "--target-dir",
            build_dir.to_str().unwrap(),
        ])
        .current_dir(root)
        .stdout(expand_out));

    (lib_name, expanded)
}

/// Build + generate a single header containing all exports.
fn build_ffi(output: &str) {
    let root = cwd();
    let (lib_name, expanded) = build_and_expand(&root);

    let cbindgen_toml = root.join("cbindgen.toml");
    let mut cbindgen_args = vec!["--output", output];
    if cbindgen_toml.exists() {
        cbindgen_args.extend(["--config", "cbindgen.toml"]);
    }
    cbindgen_args.push(expanded.to_str().unwrap());

    println!("generating {output}...");
    run(Command::new("cbindgen")
        .args(&cbindgen_args)
        .current_dir(&root));

    println!("done — {output} written, target/release/lib{lib_name}.a built");
}

/// Build + generate one header per `export_wdf!` prefix into `output_dir`.
fn build_ffi_multi(output_dir: &str) {
    let root = cwd();
    let (lib_name, expanded) = build_and_expand(&root);

    std::fs::create_dir_all(output_dir).expect("failed to create output directory");

    // Generate a combined header to a temp path first.
    let full_header_path = std::env::temp_dir().join("wdfrs_full.h");
    let cbindgen_toml = root.join("cbindgen.toml");
    let mut cbindgen_args = vec!["--output", full_header_path.to_str().unwrap()];
    if cbindgen_toml.exists() {
        cbindgen_args.extend(["--config", "cbindgen.toml"]);
    }
    cbindgen_args.push(expanded.to_str().unwrap());

    println!("generating combined header...");
    run(Command::new("cbindgen")
        .args(&cbindgen_args)
        .current_dir(&root));

    // Discover prefixes from the expanded source.
    let expanded_src = std::fs::read_to_string(&expanded).expect("failed to read expanded source");
    let prefixes = find_export_prefixes(&expanded_src);

    if prefixes.is_empty() {
        eprintln!("warning: no export_wdf! exports found — nothing to split");
        return;
    }

    let full_header =
        std::fs::read_to_string(&full_header_path).expect("failed to read generated header");
    let out_dir = Path::new(output_dir);

    println!("writing {} header(s):", prefixes.len());
    for prefix in &prefixes {
        let dest = out_dir.join(format!("{prefix}.h"));
        let content = filter_header_for_prefix(&full_header, prefix);
        std::fs::write(&dest, content).expect("failed to write header");
        println!("  {}", dest.display());
    }

    println!(
        "done — {} headers in {output_dir}, target/release/lib{lib_name}.a built",
        prefixes.len()
    );
}

/// Scan the macro-expanded source for `export_wdf!` output.
///
/// Each `export_wdf!` emits `pub extern "C" fn {prefix}_{f32|f64}_new(`.
/// Collecting unique prefixes gives us one entry per invocation.
fn find_export_prefixes(expanded: &str) -> Vec<String> {
    let mut prefixes = Vec::new();
    for line in expanded.lines() {
        let Some(rest) = line.trim().strip_prefix("pub extern \"C\" fn ") else {
            continue;
        };
        let fn_name = rest.split('(').next().unwrap_or("").trim();
        for prec in ["_f32_new", "_f64_new"] {
            if let Some(prefix) = fn_name.strip_suffix(prec) {
                let s = prefix.to_string();
                if !prefixes.contains(&s) {
                    prefixes.push(s);
                }
            }
        }
    }
    prefixes
}

/// Extract the declarations for `prefix` from a full cbindgen-generated header.
///
/// Strategy: split on blank lines to get declaration blocks, keep a block if
/// it contains `{prefix}_`.  Surround with a fresh per-prefix header guard and
/// the original `#include` lines.
fn filter_header_for_prefix(full_header: &str, prefix: &str) -> String {
    let guard = format!("{}_H", prefix.to_uppercase());
    let marker = format!("{prefix}_");

    let mut includes: Vec<&str> = Vec::new();
    let mut decls: Vec<String> = Vec::new();
    let mut block: Vec<&str> = Vec::new();

    let flush = |block: &mut Vec<&str>, marker: &str, decls: &mut Vec<String>| {
        if !block.is_empty() {
            let text = block.join("\n");
            if text.contains(marker) {
                decls.push(text);
            }
            block.clear();
        }
    };

    for line in full_header.lines() {
        let trimmed = line.trim();

        // Drop the original header guard lines; we'll write our own.
        if trimmed.starts_with("#ifndef ")
            || trimmed.starts_with("#endif")
            // `#define GUARD_H` — guard define has no space after the ident
            || (trimmed.starts_with("#define ") && !trimmed[8..].trim().contains(' '))
        {
            flush(&mut block, &marker, &mut decls);
            continue;
        }

        if trimmed.starts_with("#include") {
            flush(&mut block, &marker, &mut decls);
            includes.push(line);
            continue;
        }

        if trimmed.is_empty() {
            flush(&mut block, &marker, &mut decls);
        } else {
            block.push(line);
        }
    }
    flush(&mut block, &marker, &mut decls);

    let mut out = String::new();
    out.push_str(&format!("#ifndef {guard}\n#define {guard}\n\n"));
    for inc in &includes {
        out.push_str(inc);
        out.push('\n');
    }
    if !includes.is_empty() {
        out.push('\n');
    }
    for decl in &decls {
        out.push_str(decl);
        out.push_str("\n\n");
    }
    out.push_str(&format!("#endif /* {guard} */\n"));
    out
}
