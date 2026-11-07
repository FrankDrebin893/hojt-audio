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
            .join(
                "
",
            );
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

/// `check_drift` lists a name no file has and no other tracked file mentions, and only that one.
#[test]
fn test_the_drift_check_accepts_names_written_elsewhere() {
    use std::process::Command;

    // On Windows Git's own bash: `bash` on the PATH can be the WSL launcher, which cannot run a
    // script there. Where there is none (or git), the test has nothing to run
    let bash = if cfg!(windows) {
        let git = std::path::Path::new(r"C:\Program Files\Git\bin\bash.exe");
        if !git.exists() {
            return;
        }
        git.to_path_buf()
    } else {
        "bash".into()
    };

    let script = read("scripts/weekly-checks.sh");
    let start = script.find("check_drift() {").expect("no check_drift");
    let body = &script[start..];
    let end = body.find("\n}\n").expect("check_drift does not end") + 3;
    let function = &body[..end];

    let dir = std::env::temp_dir().join(format!("hojt-drift-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(dir.join("real.md"), "x").unwrap();
    std::fs::write(dir.join("src/main.rs"), "const URL: &str = \"releases/latest/download/\";\n")
        .unwrap();
    std::fs::write(
        dir.join("AGENTS.md"),
        "`real.md`, `releases/latest/download/`, `gone/nowhere.json`, `only/docs.sh`\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("check.sh"),
        format!("file() {{ echo \"FILED: $2\"; }}\n{function}\ncheck_drift\n"),
    )
    .unwrap();

    let run = |args: &[&str]| {
        let out = Command::new("git").args(args).current_dir(&dir).output().unwrap();
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    };
    run(&["init", "-q"]);
    run(&["add", "-A"]);

    let out = Command::new(&bash).arg("check.sh").current_dir(&dir).output().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(out.status.success(), "check_drift failed: {}", String::from_utf8_lossy(&out.stderr));
    assert!(text.contains("`gone/nowhere.json`"), "a name found nowhere is not listed:\n{text}");
    assert!(text.contains("`only/docs.sh`"), "a name only the docs mention is not listed:\n{text}");
    assert!(!text.contains("real.md"), "a real file is listed:\n{text}");
    assert!(
        !text.contains("releases/latest/download"),
        "a name in a source file is listed:\n{text}"
    );
}
