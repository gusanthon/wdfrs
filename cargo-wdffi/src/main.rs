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
//! ```sh
//! cargo wdffi output/myfilter.h
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

    if output.ends_with(".h") || output.ends_with(".hpp") {
        build_ffi(&output);
    } else {
        eprintln!(
            "error: output path {output:?} is not a header\n\n\
             hint: `cargo wdffi` writes one header containing every export:\n\n  \
             cargo wdffi output.h\n"
        );
        std::process::exit(1);
    }
}

fn help() {
    eprintln!(
        "usage:
  cargo wdffi <output.h>          — header for every export_wdf! in the crate

builds the release staticlib and generates the C header via cbindgen.

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

/// Build the crate and return the staticlib archive it produced.
///
/// The archive is *found* rather than predicted. Deriving the name from
/// `[package] name` is wrong whenever `[lib] name` differs, and a crate that
/// never declared `crate-type = ["staticlib"]` produces no archive at all —
/// both of which used to be reported as a successful build of a path nothing
/// had written, deferring the problem to a puzzling link error much later.
/// Linking this archive is half of what the tool is for, so it fails loudly
/// instead.
fn build_staticlib(root: &Path) -> PathBuf {
    println!("building release staticlib...");
    run(Command::new("cargo")
        .args(["build", "--release", "--no-default-features"])
        .current_dir(root));

    // Cargo puts the archive for this crate's lib target directly in the profile
    // directory; dependencies build under `deps/`, so anything here belongs to
    // the crate being built.
    let profile_dir = match std::env::var_os("CARGO_TARGET_DIR") {
        Some(dir) => PathBuf::from(dir),
        None => root.join("target"),
    }
    .join("release");

    let mut archives: Vec<PathBuf> = std::fs::read_dir(&profile_dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|e| e == "a"))
        .collect();
    archives.sort();

    let lib_path = match archives.as_slice() {
        [only] => only.clone(),
        [] => {
            eprintln!(
                "error: the build produced no staticlib archive in {}\n\n\
                 `cargo wdffi` builds an archive for your C/C++ project to link, so the crate \
                 has to declare one:\n\n  \
                 [lib]\n  \
                 crate-type = [\"staticlib\"]   # add \"rlib\" too if Rust code depends on this crate\n",
                profile_dir.display()
            );
            std::process::exit(1);
        }
        many => {
            // A workspace sharing one target directory across several staticlib
            // members. Guessing here is how the old code went wrong, so don't.
            eprintln!(
                "error: found {} staticlib archives in {}:\n",
                many.len(),
                profile_dir.display()
            );
            for path in many {
                eprintln!("  {}", path.display());
            }
            eprintln!("\nhint: build with a target directory of its own, e.g. CARGO_TARGET_DIR=target/ffi");
            std::process::exit(1);
        }
    };

    strip_debug_info(&lib_path);

    lib_path
}

/// Remove the DWARF sections that the profile's `strip = true` does not reach
/// for staticlib archives. Worth ~35% on a release build.
///
/// Best-effort — a missing or uncooperative `strip` is not fatal — but reported
/// rather than swallowed, since a silent failure under a message announcing
/// success is how this went unnoticed: `--strip-debug` is GNU binutils syntax
/// and Apple's `strip` rejects it, so on macOS the step did nothing at all.
fn strip_debug_info(lib_path: &Path) {
    let flag = if cfg!(target_os = "macos") {
        "-S"
    } else {
        "--strip-debug"
    };

    let name = lib_path.file_name().unwrap_or_default().to_string_lossy();
    println!("stripping debug info from {name}...");

    match Command::new("strip")
        .args([flag, lib_path.to_str().unwrap()])
        .status()
    {
        Ok(status) if status.success() => {}
        Ok(status) => eprintln!("warning: `strip {flag}` exited with status {status}"),
        Err(e) => eprintln!("warning: could not run `strip`: {e}"),
    }
}

/// Shared first two steps: build the staticlib, then `cargo expand`.
///
/// Returns `(path_to_archive, path_to_expanded_rs)`.
fn build_and_expand(root: &Path) -> (PathBuf, PathBuf) {
    let lib_path = build_staticlib(root);

    let expanded = std::env::temp_dir().join("wdfrs_expanded.rs");
    let build_dir = std::env::temp_dir().join("wdfrs_build");

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

    (lib_path, expanded)
}

/// Build + generate a single header containing all exports.
fn build_ffi(output: &str) {
    let root = cwd();
    let (lib_path, expanded) = build_and_expand(&root);

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

    println!("done — {output} written, {} built", lib_path.display());
}
