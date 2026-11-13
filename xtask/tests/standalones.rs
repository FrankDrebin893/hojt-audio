//! The six standalone programs ship in the Windows installer: each has the HØJT icon in its exe
//! (the same badge, drawn once by `scripts/make-icon.py`), a component and a Start menu entry in
//! `installer/hojt-dsp.iss`, and is built and started in the Windows job of `release.yml`.
//! ASIO is not part of it: that stays a build by hand (`plugins/amp/CLAUDE.md`)

use std::path::Path;

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap()
}

fn read(path: &str) -> String {
    std::fs::read_to_string(root().join(path)).unwrap().replace("\r\n", "\n")
}

/// (package, bundle name) of each section of bundler.toml
fn bundles() -> Vec<(String, String)> {
    let toml = read("bundler.toml");
    let mut out = Vec::new();
    let mut package = String::new();
    for line in toml.lines() {
        if let Some(p) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            package = p.to_string();
        } else if let Some(n) = line.strip_prefix("name = \"").and_then(|l| l.strip_suffix('"')) {
            out.push((package.clone(), n.to_string()));
        }
    }
    out
}

#[test]
fn test_every_plugin_gives_its_standalone_exe_the_badge() {
    let bundles = bundles();
    assert!(bundles.len() >= 6, "{bundles:?}");
    let amp_ico = std::fs::read(root().join("plugins/amp/assets/amp.ico")).unwrap();
    for (package, _) in &bundles {
        let dir = format!("plugins/{package}");
        let ico = std::fs::read(root().join(format!("{dir}/assets/{package}.ico"))).unwrap_or_else(
            |_| panic!("{package} has no assets/{package}.ico (scripts/make-icon.py)"),
        );
        assert_eq!(
            ico, amp_ico,
            "{package}.ico is not the badge of scripts/make-icon.py (run it again)"
        );
        assert_eq!(
            read(&format!("{dir}/assets/{package}.rc")).trim(),
            format!("1 ICON \"{package}.ico\"")
        );
        let build: String = read(&format!("{dir}/build.rs")).split_whitespace().collect();
        assert!(
            build.contains(&format!(
                "compile_for(\"assets/{package}.rc\",[\"{package}_standalone\"]"
            )),
            "{package}'s build.rs does not put the icon in {package}_standalone"
        );
        let cargo = read(&format!("{dir}/Cargo.toml"));
        assert!(cargo.contains("embed-resource"), "{package} does not depend on embed-resource");
    }
}

#[test]
fn test_every_standalone_is_in_the_installer_with_a_start_menu_entry() {
    let script = read("installer/hojt-dsp.iss");
    let ids = ["pitch_shifter", "drums", "amp", "bass", "pad", "bells"];
    for (package, name) in bundles() {
        assert!(ids.contains(&package.as_str()), "a new plugin {package}: add it to this test");
        let component = format!("{package}\\standalone");
        assert!(
            script.contains(&format!("Name: \"{component}\"; Description: \"Standalone program\"")),
            "{package} has no standalone component"
        );
        let files: Vec<&str> = script
            .lines()
            .filter(|l| l.starts_with("Source:") && l.contains(&format!("\\{name}.exe\"")))
            .collect();
        assert_eq!(files.len(), 1, "{name}.exe needs one [Files] line");
        assert!(files[0].contains(&format!("Components: {component};")), "{}", files[0]);
        assert!(files[0].contains("DestDir: \"{app}\""), "{}", files[0]);
        assert!(
            script.lines().any(|l| l.starts_with("Name: \"{group}\\")
                && l.contains(&format!("Filename: \"{{app}}\\{name}.exe\""))
                && l.contains(&format!("Components: {component}"))),
            "{name}.exe has no Start menu entry"
        );
    }
}

#[test]
fn test_the_windows_job_builds_and_starts_every_standalone() {
    let release = read(".github/workflows/release.yml");
    assert!(release.contains("scripts/build-standalones.sh"), "release.yml builds no standalones");
    assert!(release.contains("scripts/smoke-standalones.ps1"), "release.yml starts no standalones");
    // Before the installer, which packs them
    let standalones = release.find("scripts/build-standalones.sh").unwrap();
    let installer = release.find("run: bash scripts/build-installer.sh").unwrap();
    assert!(standalones < installer, "the standalones are built after the installer");
    let build = read("scripts/build-installer.sh");
    assert!(
        build.contains("target/standalone"),
        "build-installer.sh does not read target/standalone"
    );
    assert!(read("installer/test-installer.ps1").contains("Assert-Standalones"));
}
