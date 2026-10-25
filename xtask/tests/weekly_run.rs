//! The weekly run (`.github/workflows/weekly.yml`, `scripts/weekly-checks.sh`) only reports and
//! files ideas: it runs on a schedule and by hand, holds no more than the right to write issues,
//! uses no secret but the built-in token, and neither the workflow nor the script ever sets
//! `state:approved` (only Rasmus does; AGENTS.md, "How work flows")

use std::path::Path;

fn read(path: &str) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    std::fs::read_to_string(root.join(path)).unwrap().replace("\r\n", "\n")
}

#[test]
fn test_the_workflow_runs_weekly_and_by_hand() {
    let yml = read(".github/workflows/weekly.yml");
    assert!(yml.contains("schedule:") && yml.contains("cron:"), "no weekly schedule");
    assert!(yml.contains("workflow_dispatch:"), "cannot be run by hand");
    assert!(yml.contains("scripts/weekly-checks.sh"), "does not run the script");
}

#[test]
fn test_the_workflow_holds_the_least_rights_and_no_key() {
    let yml = read(".github/workflows/weekly.yml");
    assert!(yml.contains("contents: read") && yml.contains("issues: write"));
    for forbidden in ["pull-requests: write", "contents: write", "actions: write", "write-all"] {
        assert!(!yml.contains(forbidden), "weekly.yml holds `{forbidden}`");
    }
    let secrets: Vec<_> = yml.match_indices("secrets.").map(|(i, _)| &yml[i..i + 20]).collect();
    assert!(
        secrets.iter().all(|s| s.starts_with("secrets.GITHUB_TOKEN")),
        "weekly.yml uses a secret other than the built-in token: {secrets:?}"
    );
}

#[test]
fn test_the_run_never_approves_builds_or_merges() {
    for path in [".github/workflows/weekly.yml", "scripts/weekly-checks.sh"] {
        // comments may say what the run does not do
        let text: String = read(path)
            .lines()
            .filter(|line| !line.trim_start().starts_with('#'))
            .collect::<Vec<_>>()
            .join("
");
        for forbidden in ["state:approved", "gh pr merge", "gh pr create", "git push", "--auto"] {
            assert!(!text.contains(forbidden), "{path} contains `{forbidden}`");
        }
    }
}

#[test]
fn test_the_script_has_a_dry_run_and_files_ideas() {
    let script = read("scripts/weekly-checks.sh");
    assert!(script.contains("--dry-run"), "no dry run");
    assert!(script.contains("--label state:idea"), "does not file state:idea issues");
}
