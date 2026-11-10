//! The Windows installer (`installer/hojt-dsp.iss`) carries every bundle of `bundler.toml`, in both
//! formats, under a component of its own, and keeps the `AppId` a newer setup upgrades by

use std::path::Path;

fn read(path: &str) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    std::fs::read_to_string(root.join(path)).unwrap().replace("\r\n", "\n")
}

fn bundle_names(bundler: &str) -> Vec<String> {
    bundler
        .lines()
        .filter_map(|line| line.strip_prefix("name = \"")?.strip_suffix('"').map(String::from))
        .collect()
}

/// The `[Files]` lines of the script that name `file`
fn file_lines<'a>(script: &'a str, file: &str) -> Vec<&'a str> {
    script
        .lines()
        .filter(|line| line.starts_with("Source:") && line.contains(&format!("\\{file}")))
        .collect()
}

#[test]
fn test_every_bundle_is_in_the_installer_in_both_formats() {
    let script = read("installer/hojt-dsp.iss");
    let names = bundle_names(&read("bundler.toml"));
    assert!(names.len() >= 6, "found too few bundles in bundler.toml: {names:?}");
    let mut wrong = Vec::new();
    for name in &names {
        for (format, dest) in [("vst3", "{code:PluginDir|VST3}"), ("clap", "{code:PluginDir|CLAP}")]
        {
            let lines = file_lines(&script, &format!("{name}.{format}"));
            if lines.len() != 1 {
                wrong.push(format!(
                    "{name}.{format} has {} line(s) in [Files], not one",
                    lines.len()
                ));
            } else if !lines[0].contains(dest) {
                wrong.push(format!("{name}.{format} does not go to {dest}"));
            } else if !lines[0].contains("Components:") {
                wrong.push(format!("{name}.{format} is not under a component"));
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "{}\n\nEvery bundle of bundler.toml gets a VST3 and a CLAP line in installer/hojt-dsp.iss, and a component",
        wrong.join("\n")
    );
}

#[test]
fn test_every_component_has_files_and_every_file_a_component() {
    let script = read("installer/hojt-dsp.iss");
    let components: Vec<&str> = script
        .lines()
        .skip_while(|line| *line != "[Components]")
        .skip(1)
        .take_while(|line| !line.starts_with('['))
        .filter_map(|line| line.strip_prefix("Name: \"")?.split('"').next())
        .collect();
    assert!(components.len() >= 18, "six plugins with VST3 and CLAP each: {components:?}");
    for component in &components {
        if component.contains('\\') {
            assert!(
                script.contains(&format!("Components: {component};")),
                "component {component} installs nothing"
            );
        }
    }
    for line in
        script.lines().filter(|line| line.starts_with("Source:") && line.contains("PluginDir"))
    {
        let used = line.split("Components: ").nth(1).and_then(|rest| rest.split(';').next());
        assert!(
            used.is_some_and(|used| components.contains(&used)),
            "a file line names no component of the page: {line}"
        );
    }
}

#[test]
fn test_the_app_id_is_the_one_upgrades_find_it_by() {
    // Changing it makes the next setup install beside the old one instead of over it, and
    // installer/test-installer.ps1 looks the install up by it
    let script = read("installer/hojt-dsp.iss");
    let test = read("installer/test-installer.ps1");
    assert!(script.contains("AppId={{4DFF5B9E-7443-46F9-9E6B-18D7A4889F6E}\n"));
    assert!(test.contains("'{4DFF5B9E-7443-46F9-9E6B-18D7A4889F6E}'"));
    assert!(script.contains("PrivilegesRequired=admin"));
    assert!(script.contains("CloseApplications=yes"));
}

#[test]
fn test_the_installer_is_built_and_tested_in_the_windows_job() {
    let release = read(".github/workflows/release.yml");
    assert!(
        release.contains("scripts/build-installer.sh"),
        "release.yml does not build the installer"
    );
    assert!(
        release.contains("installer/test-installer.ps1"),
        "release.yml does not test the installer"
    );
    assert!(release.contains("scripts/checksums.sh"), "release.yml publishes no SHA256SUMS.txt");
    for path in ["installer/**", "scripts/build-installer.sh", "scripts/checksums.sh"] {
        assert!(
            release.contains(path),
            "a change to {path} does not run release.yml on a pull request"
        );
    }
}
