use std::path::{Path, PathBuf};

// nih_plug_xtask takes the outermost folder with a Cargo.toml for the workspace. A git worktree
// under .claude/worktrees sits inside the main checkout, so it would build and bundle the main
// checkout's sources instead of its own. Bundling is done here, from this workspace's root.
fn main() -> nih_plug_xtask::Result<()> {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("xtask lives in the workspace root");
    std::env::set_current_dir(root)?;

    let mut args = std::env::args().skip(1);
    let command = args.next().unwrap_or_default();
    let universal = match command.as_str() {
        "bundle" => false,
        "bundle-universal" => true,
        _ => return nih_plug_xtask::main(),
    };
    let (packages, other_args) = split_bundle_args(args)?;
    let target_dir = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("target"));

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
        packages.push(args.next().ok_or_else(|| missing("Missing package name after -p"))?);
    }
    if packages.is_empty() {
        packages.push(
            args.next()
                .ok_or_else(|| missing("Missing package name: cargo xtask bundle <package>"))?,
        );
    }
    Ok((packages, args.collect()))
}

fn missing(message: &str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidInput, message)
}
