//! `cargo wdffi` — build FFI artifacts for any crate that uses `export_wdf!`.
//!
//! This cargo subcommand automates the two-step process of exposing a wdfrs
//! circuit as a C-compatible library. Run it from the root of your own crate
//! (the one that calls `export_wdf!`), not from the wdfrs workspace itself.
//!
//! 1. **Build** — compiles your crate as a `staticlib`.
//! 2. **Header generation** — macro-expands your crate with `cargo expand` and
//!    feeds the result to `cbindgen` to produce a C header.
//!
//! Both steps take the same profile and the same feature selection, so the
//! header always describes the archive that was just built.
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
//! cargo wdffi uv.h --profile plugin        # link a custom profile
//! cargo wdffi uv.h --no-default-features   # applies to build and expand alike
//! ```
//!
//! # cbindgen configuration
//!
//! A `cbindgen.toml` in your crate root is used if present. Otherwise a default
//! is written into the run's temp directory and used, equivalent to:
//!
//! ```toml
//! language = "C"
//! include_version = true
//! include_guard = "MYFILTER_H"   # from the output filename
//! cpp_compat = true
//!
//! [export]
//! item_types = ["functions"]
//! ```
//!
//! `item_types` is the load-bearing line: without it cbindgen also emits every
//! `pub const` in the expanded crate as a `#define`, which then leaks into each
//! translation unit that includes the header. `cpp_compat` supplies the
//! `extern "C"` wrapper C++ callers need.
//!
//! # Why `nm` shows no symbols
//!
//! With LTO enabled — `lto = "thin"` or `"fat"` in the profile you built — the
//! archive members are LLVM bitcode rather than object code, and Apple's `nm`
//! rejects them outright:
//!
//! ```text
//! nm: error: …rcgu.o: Unknown attribute kind (102)
//! ```
//!
//! An empty symbol listing is therefore not evidence of a missing export. `ld64`
//! links the archive without complaint; only the inspection tools cannot read
//! it. Use the toolchain's own `llvm-nm` instead:
//!
//! ```sh
//! "$(rustc --print sysroot)"/lib/rustlib/*/bin/llvm-nm myfilter.a
//! ```

use std::path::{Path, PathBuf};
use std::process::Command;

fn cwd() -> PathBuf {
    std::env::current_dir().expect("failed to get current directory")
}

/// What the caller asked for.
///
/// The feature flags are kept verbatim so that one selection reaches both
/// `cargo build` and `cargo expand`. When the two disagree the header can
/// describe a configuration the archive does not contain, which surfaces as
/// missing symbols at link time rather than as an error here.
struct Options {
    output: String,
    profile: String,
    features: Vec<String>,
}

fn main() {
    // When invoked as `cargo wdffi <arg>`, cargo passes "wdffi" as the first
    // argument. Skip it so both `cargo-wdffi <arg>` and `cargo wdffi <arg>`
    // work identically.
    let mut args = std::env::args().skip(1).peekable();
    if args.peek().map(String::as_str) == Some("wdffi") {
        args.next();
    }

    let options = parse_options(args);

    if !(options.output.ends_with(".h") || options.output.ends_with(".hpp")) {
        eprintln!(
            "error: output path {:?} is not a header\n\n\
             hint: `cargo wdffi` writes one header containing every export:\n\n  \
             cargo wdffi output.h\n",
            options.output
        );
        std::process::exit(1);
    }

    build_ffi(&options);
}

fn parse_options(mut args: impl Iterator<Item = String>) -> Options {
    let output = args.next().unwrap_or_else(|| {
        help();
        std::process::exit(1);
    });

    let mut profile = None;
    let mut features = Vec::new();

    while let Some(arg) = args.next() {
        let mut value = || {
            args.next().unwrap_or_else(|| {
                eprintln!("error: {arg} needs a value");
                std::process::exit(1);
            })
        };

        match arg.as_str() {
            "--profile" => profile = Some(value()),
            "--release" => profile = Some("release".to_string()),
            "--features" | "-F" => {
                features.push("--features".to_string());
                features.push(value());
            }
            "--all-features" => features.push("--all-features".to_string()),
            "--no-default-features" => features.push("--no-default-features".to_string()),
            other => {
                eprintln!("error: unknown option {other:?}\n");
                help();
                std::process::exit(1);
            }
        }
    }

    Options {
        output,
        profile: profile.unwrap_or_else(|| "release".to_string()),
        features,
    }
}

fn help() {
    eprintln!(
        "usage:
  cargo wdffi <output.h> [options]   — header for every export_wdf! in the crate

options:
  --profile <name>         profile to build and expand with [default: release]
  --release                same as --profile release
  --features <list>        forwarded to both the build and the expansion
  --all-features           likewise
  --no-default-features    likewise

builds the staticlib and generates the C header via cbindgen. A cbindgen.toml in
the crate root is used if present, otherwise a default is supplied.

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

/// The directory cargo writes a profile's artifacts to. Only the built-in
/// profiles disagree with their own names; a custom `[profile.plugin]` lands in
/// `target/plugin`.
fn profile_dir_name(profile: &str) -> &str {
    match profile {
        "dev" | "test" => "debug",
        "bench" => "release",
        other => other,
    }
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
fn build_staticlib(root: &Path, options: &Options) -> PathBuf {
    println!("building {} staticlib...", options.profile);
    run(Command::new("cargo")
        .args(["build", "--profile", &options.profile])
        .args(&options.features)
        .current_dir(root));

    // Cargo puts the archive for this crate's lib target directly in the profile
    // directory; dependencies build under `deps/`, so anything here belongs to
    // the crate being built.
    let profile_dir = match std::env::var_os("CARGO_TARGET_DIR") {
        Some(dir) => PathBuf::from(dir),
        None => root.join("target"),
    }
    .join(profile_dir_name(&options.profile));

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

/// The lib name cargo used, read back off the archive it wrote.
fn lib_name_from(lib_path: &Path) -> String {
    let stem = lib_path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();

    stem.strip_prefix("lib").unwrap_or(&stem).to_string()
}

/// A directory of this run's own, under the system temp dir.
///
/// Fixed names in a shared directory mean concurrent runs in different crates
/// clobber each other's expansion, and on a multi-user machine another user's
/// file blocks you outright. Removed on success; left behind on failure, where
/// the expansion is the thing you want to look at.
fn scratch_dir(lib_name: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);

    let dir = std::env::temp_dir().join(format!(
        "cargo-wdffi-{lib_name}-{}-{nanos}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("failed to create temp directory");

    dir
}

/// Shared first two steps: build the staticlib, then `cargo expand`.
///
/// The expanded file is named after the crate's lib target, so cbindgen's
/// diagnostics name a module the caller recognises.
///
/// Returns `(path_to_archive, scratch_dir, path_to_expanded_rs)`.
fn build_and_expand(root: &Path, options: &Options) -> (PathBuf, PathBuf, PathBuf) {
    let lib_path = build_staticlib(root, options);

    let lib_name = lib_name_from(&lib_path);
    let scratch = scratch_dir(&lib_name);
    let expanded = scratch.join(format!("{lib_name}.rs"));

    println!("expanding macros...");
    let expand_out = std::fs::File::create(&expanded).expect("failed to create temp file");
    run(Command::new("cargo")
        .args(["expand", "--lib", "--profile", &options.profile])
        .args(&options.features)
        .args(["--target-dir", scratch.join("build").to_str().unwrap()])
        .current_dir(root)
        .stdout(expand_out));

    (lib_path, scratch, expanded)
}

/// The cbindgen config to use: the crate's own if it has one, otherwise a
/// default written into `scratch`.
///
/// Nothing is written into the caller's crate — a default belongs in this run,
/// not in their tree, and a committed `cbindgen.toml` always wins. The two
/// settings that only matter downstream are `item_types`, without which every
/// `pub const` in the expanded crate becomes a `#define`, and `cpp_compat`,
/// without which the declarations mangle when included from C++.
fn cbindgen_config(root: &Path, scratch: &Path, output: &str) -> PathBuf {
    let own = root.join("cbindgen.toml");
    if own.exists() {
        return own;
    }

    let guard = Path::new(output)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect::<String>()
        .to_uppercase();

    let path = scratch.join("cbindgen.toml");
    std::fs::write(
        &path,
        format!(
            "language = \"C\"\n\
             include_version = true\n\
             include_guard = \"{guard}_H\"\n\
             cpp_compat = true\n\
             \n\
             [export]\n\
             item_types = [\"functions\"]\n"
        ),
    )
    .expect("failed to write default cbindgen.toml");

    println!("no cbindgen.toml in the crate root — using the built-in default");

    path
}

/// Build + generate a single header containing all exports.
fn build_ffi(options: &Options) {
    let root = cwd();
    let output = &options.output;
    let (lib_path, scratch, expanded) = build_and_expand(&root, options);

    let config = cbindgen_config(&root, &scratch, output);

    println!("generating {output}...");
    run(Command::new("cbindgen")
        .args(["--output", output])
        .args(["--config", config.to_str().unwrap()])
        .arg(expanded.to_str().unwrap())
        .current_dir(&root));

    let _ = std::fs::remove_dir_all(&scratch);

    println!("done — {output} written, {} built", lib_path.display());
}
