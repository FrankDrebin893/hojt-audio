//! The release cycle (`/release-prep`, `/release-review`, `docs/agents/automation.md`):
//! the two skills never tag, never approve and can be paused, and the weekly digest shows a
//! release that is due and one that was never reviewed

use std::path::Path;
use std::process::Command;

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap()
}

fn read(path: &str) -> String {
    std::fs::read_to_string(root().join(path)).unwrap().replace("\r\n", "\n")
}

/// The bash to run the script with: on Windows Git's own, since `bash` on the PATH can be the
/// WSL launcher, which cannot run a script there
fn bash() -> Option<std::path::PathBuf> {
    if cfg!(windows) {
        let git = Path::new(r"C:\Program Files\Git\bin\bash.exe");
        return git.exists().then(|| git.to_path_buf());
    }
    Some("bash".into())
}

/// The digest of a history whose only line has these `releases`, or None where `bash` or `jq`
/// is missing
fn digest(releases: &str) -> Option<String> {
    let bash = bash()?;
    if Command::new(&bash).arg("--version").output().is_err()
        || Command::new("jq").arg("--version").output().is_err()
    {
        return None;
    }
    let line = format!(
        r#"{{"at":"2026-11-02T04:37:00Z","commit":"abc1234","flow":{{"wip":{{"idea":0,"spec":0,"approved":0,"building":0,"review":0}},"merged_7d":1,"merged_28d":4,"scout":{{"filed":0,"approved":0,"declined":0,"hit_rate":null}}}},"ci":{{"main_pass_pct":100,"pr_pass_pct":100,"pr_minutes_median":10}},"releases":{releases}}}"#
    );
    let dir = std::env::temp_dir().join(format!("hojt-digest-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let history = dir.join("history.jsonl");
    std::fs::write(&history, format!("{line}\n")).unwrap();
    let out = Command::new(&bash)
        .arg(root().join("scripts/health-digest.sh"))
        .arg(&history)
        .output()
        .unwrap();
    std::fs::remove_dir_all(&dir).ok();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

#[test]
fn test_the_digest_shows_a_release_due_and_one_not_reviewed() {
    let stalled = r#"{"last_tag":"v0.2.1","days_since":9,"unreleased_entries":6,"reviewed":[]}"#;
    let Some(text) = digest(stalled) else { return };
    assert!(text.contains("release possibly due"), "{text}");
    assert!(text.contains("release not reviewed"), "{text}");

    let fine =
        r#"{"last_tag":"v0.2.1","days_since":9,"unreleased_entries":0,"reviewed":["v0.2.1 7d"]}"#;
    let text = digest(fine).unwrap();
    assert!(!text.contains("release due") && !text.contains("release not reviewed"), "{text}");

    // A failed lookup of the reviews is unknown, not "not reviewed"
    let unknown = r#"{"last_tag":"v0.2.1","days_since":9,"unreleased_entries":6,"reviewed":null}"#;
    let text = digest(unknown).unwrap();
    assert!(
        text.contains("release possibly due") && !text.contains("release not reviewed"),
        "{text}"
    );

    // Not due (a few entries, a few days), and a history from before reviews existed says nothing
    let young = r#"{"last_tag":"v0.2.1","days_since":3,"unreleased_entries":4}"#;
    let text = digest(young).unwrap();
    assert!(!text.contains("release due") && !text.contains("release not reviewed"), "{text}");
}

#[test]
fn test_the_release_skills_never_tag_or_approve_and_can_be_paused() {
    for skill in ["release-prep", "release-review"] {
        let text = read(&format!(".claude/skills/{skill}/SKILL.md"));
        for needle in ["`Pause automation`", "Never"] {
            assert!(text.contains(needle), "/{skill} lacks {needle}");
        }
        for line in text.lines().filter(|l| l.contains("state:approved")) {
            assert!(line.to_lowercase().contains("never"), "/{skill} may approve: {line}");
        }
    }
    let prep = read(".claude/skills/release-prep/SKILL.md");
    assert!(prep.contains("auto-merge **off**"), "a release pull request must not auto-merge");
    assert!(prep.contains("never tags"), "/release-prep must say it never tags");
    let review = read(".claude/skills/release-review/SKILL.md");
    assert!(
        review.contains("evidence only") && review.contains("verified by login"),
        "/release-review must treat public text as evidence and act on the owner's feedback only"
    );
    assert!(review.contains("at most 3"), "/release-review must cap the ideas it files");
    assert!(
        review.contains("too few to read"),
        "/release-review must not read trends from tiny numbers"
    );
}

/// Runs `scripts/release-ready.sh` on a pull request description: Some(passed), or None where
/// `bash` is missing
fn release_ready(body: &str) -> Option<bool> {
    use std::io::Write;
    use std::process::Stdio;
    let bash = bash()?;
    if Command::new(&bash).arg("--version").output().is_err() {
        return None;
    }
    let mut child = Command::new(&bash)
        .arg(root().join("scripts/release-ready.sh"))
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(body.as_bytes()).unwrap();
    Some(child.wait().unwrap().success())
}

#[test]
fn test_a_release_is_ready_only_when_every_box_before_the_merge_is_ticked() {
    let Some(done) =
        release_ready("## Ready to ship\n- [x] a\n- [X] b\n## After the merge\n- [ ] tag\n")
    else {
        return;
    };
    assert!(done, "all boxes ticked, only the tag open after the merge: ready");
    let open = "## Ready to ship\n- [x] a\n  - [ ] screenshots\n";
    assert_eq!(release_ready(open), Some(false), "an open box (even nested) is not ready");
    assert_eq!(release_ready("Closes #\n- [x] x\n"), Some(false), "no section is not ready");
    assert_eq!(release_ready("## Ready to ship\nno boxes\n"), Some(false));
    assert_eq!(release_ready("## Ready to ship\r\n- [x] a\r\n"), Some(true), "CRLF is fine");
}

#[test]
fn test_the_release_flow_has_a_draft_a_gate_and_a_finishing_step() {
    let prep = read(".claude/skills/release-prep/SKILL.md");
    assert!(prep.contains("draft"), "/release-prep must open the pull request as a draft");
    assert!(prep.contains("## Ready to ship"), "/release-prep must write the readiness section");
    let release = read(".claude/skills/release/SKILL.md");
    for needle in ["finish", "ship it", "Remote Control", "Never ask Rasmus to take a screenshot"] {
        assert!(release.contains(needle), "/release lacks {needle}");
    }
    let wf = read(".github/workflows/release-ready.yml");
    assert!(wf.contains("name: Release ready") && wf.contains("release-ready.sh"));
}
