use std::path::{Path, PathBuf};

// The pictures each plugin is drawn with, as in `suite_common::ui::artwork` and the editors.
// `check-art` looks for these in the bundles: a plugin built from other sources than these
// (another checkout, an old build left in the target folder) does not carry them
const ART: [(&str, &[&str]); 6] = [
    ("pitch_shifter", &["pitch_shifter.png"]),
    ("drums", &["drums.png"]),
    ("amp", &["amp.png", "mini.png"]),
    ("bass", &["bass.png"]),
    ("pad", &["pad.png"]),
    ("bells", &["bells.png"]),
];

// How much of a picture is looked for, from its middle: compressed pixels, so no two pictures
// share it, and short enough to find quickly in a bundle of several megabytes
const NEEDLE_LEN: usize = 256;

// nih_plug_xtask takes the outermost folder with a Cargo.toml for the workspace. A git worktree
// under .claude/worktrees sits inside the main checkout, so it would build and bundle the main
// checkout's sources instead of its own. Bundling is done here, from this workspace's root.
fn main() -> nih_plug_xtask::Result<()> {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("xtask lives in the workspace root");
    std::env::set_current_dir(root)?;

    let mut args = std::env::args().skip(1);
    let command = args.next().unwrap_or_default();
    let target_dir = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("target"));
    let universal = match command.as_str() {
        "bundle" => false,
        "bundle-universal" => true,
        "check-art" => return check_art(root, &target_dir.join("bundled"), args.collect()),
        _ => return nih_plug_xtask::main(),
    };
    let (packages, other_args) = split_bundle_args(args)?;

    if universal {
        for target in ["x86_64-apple-darwin", "aarch64-apple-darwin"] {
            let mut target_args = other_args.clone();
            target_args.push(format!("--target={target}"));
            nih_plug_xtask::build(&packages, &target_args)?;
        }
    } else {
        nih_plug_xtask::build(&packages, &other_args)?;
    }
    for package in &packages {
        nih_plug_xtask::bundle(&target_dir, package, &other_args, universal)?;
    }
    Ok(())
}

// `<package> [args]`, or `-p <package> -p <package> ... [args]`, as nih_plug_xtask takes them
fn split_bundle_args(
    args: impl Iterator<Item = String>,
) -> nih_plug_xtask::Result<(Vec<String>, Vec<String>)> {
    let mut args = args.peekable();
    let mut packages = Vec::new();
    while args.peek().map(String::as_str) == Some("-p") {
        args.next();
        packages.push(args.next().ok_or_else(|| invalid("Missing package name after -p"))?);
    }
    if packages.is_empty() {
        packages.push(
            args.next()
                .ok_or_else(|| invalid("Missing package name: cargo xtask bundle <package>"))?,
        );
    }
    Ok((packages, args.collect()))
}

/// `cargo xtask check-art [package ...]`: every VST3 and CLAP bundle of these plugins (all of
/// them when none are named) in `bundled` carries the plugin's pictures. Fails on the first
/// one that is missing or does not
fn check_art(root: &Path, bundled: &Path, packages: Vec<String>) -> nih_plug_xtask::Result<()> {
    let bundle_names = std::fs::read_to_string(root.join("bundler.toml"))?;
    let packages: Vec<&str> = if packages.is_empty() {
        ART.iter().map(|(package, _)| *package).collect()
    } else {
        packages.iter().map(String::as_str).collect()
    };
    for package in packages {
        let pictures = ART
            .iter()
            .find(|(name, _)| *name == package)
            .map(|(_, pictures)| *pictures)
            .ok_or_else(|| invalid(&format!("No pictures known for '{package}'")))?;
        let name = bundle_name(&bundle_names, package)
            .ok_or_else(|| invalid(&format!("'{package}' has no name in bundler.toml")))?;
        for format in ["vst3", "clap"] {
            let bundle = bundled.join(format!("{name}.{format}"));
            if !bundle.exists() {
                return Err(invalid(&format!("{} is missing", bundle.display())).into());
            }
            let contents = read_all(&bundle)?;
            for picture in pictures {
                let png = std::fs::read(root.join("crates/suite_common/assets/art").join(picture))?;
                let middle = png.len() / 2;
                let needle = &png[middle..(middle + NEEDLE_LEN).min(png.len())];
                if !contents.windows(needle.len()).any(|window| window == needle) {
                    return Err(invalid(&format!(
                        "{} does not carry {picture}: it was not built from this checkout's \
                         sources",
                        bundle.display()
                    ))
                    .into());
                }
            }
            println!("{name}.{format}: {}", pictures.join(", "));
        }
    }
    Ok(())
}

// The `name` under `[package]` in bundler.toml
fn bundle_name(bundler_toml: &str, package: &str) -> Option<String> {
    let mut lines = bundler_toml.lines().map(str::trim);
    lines.find(|line| *line == format!("[{package}]"))?;
    let line = lines.find(|line| line.starts_with("name"))?;
    Some(line.split('"').nth(1)?.to_string())
}

// A bundle is one file (a CLAP on Windows and Linux) or a folder: all its files, one after the
// other
fn read_all(path: &Path) -> std::io::Result<Vec<u8>> {
    if path.is_file() {
        return std::fs::read(path);
    }
    let mut contents = Vec::new();
    for entry in std::fs::read_dir(path)? {
        contents.extend(read_all(&entry?.path())?);
    }
    Ok(contents)
}

fn invalid(message: &str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidInput, message)
}
