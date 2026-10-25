//! `scripts/check.sh` runs what CI runs (Testing in scripts/README.md): every `cargo` step of
//! `.github/workflows/ci.yml` is a step of the script, and the Linux packages it checks for, which
//! it reads from the first `apt-get install` of `ci.yml`, are those of every job

use std::path::Path;

fn read(path: &str) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    std::fs::read_to_string(root.join(path)).unwrap().replace("\r\n", "\n")
}

/// The `cargo` commands of the `run:` steps, without the `cargo`.
fn ci_cargo_steps(ci: &str) -> Vec<String> {
    ci.lines()
        .filter_map(|line| line.trim().strip_prefix("run: cargo "))
        .map(|rest| rest.trim().to_string())
        .collect()
}

/// The `cargo` commands of the script's steps, without the `cargo` and the toolchain.
fn script_steps(script: &str) -> Vec<String> {
    script
        .lines()
        .filter_map(|line| line.trim().strip_prefix("step cargo "))
        .map(|rest| rest.trim().trim_start_matches("$tc ").to_string())
        .collect()
}

#[test]
fn test_every_cargo_step_of_ci_is_a_step_of_the_check() {
    let steps = script_steps(&read("scripts/check.sh"));
    let ci = ci_cargo_steps(&read(".github/workflows/ci.yml"));
    assert!(ci.len() >= 4, "found too few cargo steps in ci.yml: {ci:?}");
    let missing: Vec<_> = ci.iter().filter(|step| !steps.contains(step)).collect();
    assert!(
        missing.is_empty(),
        "scripts/check.sh does not run these steps of ci.yml: {missing:?}; add a `step cargo ...` for each"
    );
}

#[test]
fn test_every_job_installs_the_same_linux_packages() {
    let ci = read(".github/workflows/ci.yml");
    let lists: Vec<&str> = ci
        .lines()
        .filter_map(|line| line.split_once("apt-get install -y").map(|(_, list)| list.trim()))
        .collect();
    assert!(
        !lists.is_empty(),
        "ci.yml installs no Linux packages; scripts/check.sh reads them from there"
    );
    assert!(
        lists.iter().all(|list| *list == lists[0]),
        "the jobs of ci.yml install different Linux packages, but scripts/check.sh checks only the first list"
    );
}

#[test]
fn test_steps_are_read_as_written() {
    let ci = "      - name: Test\n        run: cargo test --workspace --locked\n        run: |\n";
    assert_eq!(ci_cargo_steps(ci), ["test --workspace --locked"]);
    let script = "    step cargo $tc fmt --all --check\n    step cargo test -p amp\n";
    assert_eq!(script_steps(script), ["fmt --all --check", "test -p amp"]);
}
