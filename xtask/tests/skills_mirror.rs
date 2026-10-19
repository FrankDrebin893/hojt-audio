//! `.claude/commands/` mirrors `.agents/skills/` for Claude Code (Layout in AGENTS.md): each
//! command is its skill's `SKILL.md` without the `name:` line, and there is no other command

use std::path::{Path, PathBuf};

#[test]
fn test_commands_mirror_the_skills() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let skills = root.join(".agents/skills");
    let commands = root.join(".claude/commands");
    let mut wrong = Vec::new();

    let mut names = Vec::new();
    for entry in std::fs::read_dir(&skills).unwrap() {
        let dir = entry.unwrap().path();
        let Ok(skill) = std::fs::read_to_string(dir.join("SKILL.md")) else { continue };
        let name = dir.file_name().unwrap().to_string_lossy().into_owned();
        let command = commands.join(format!("{name}.md"));
        let expected = mirror(&skill, &name);
        match std::fs::read_to_string(&command) {
            Ok(text) if text.replace("\r\n", "\n") == expected => {}
            Ok(_) => wrong.push(format!(".claude/commands/{name}.md differs from its skill")),
            Err(_) => wrong.push(format!(".claude/commands/{name}.md is missing")),
        }
        names.push(name);
    }
    for entry in std::fs::read_dir(&commands).unwrap() {
        let path: PathBuf = entry.unwrap().path();
        let name = path.file_stem().unwrap().to_string_lossy().into_owned();
        if !names.contains(&name) {
            wrong.push(format!(".claude/commands/{name}.md has no skill in .agents/skills/"));
        }
    }

    assert!(
        wrong.is_empty(),
        "{}\n\nA skill is written in `.agents/skills/<name>/SKILL.md` and mirrored for Claude Code: \
         copy it to `.claude/commands/<name>.md` and delete its `name: <name>` line. A command \
         without a skill is either moved into `.agents/skills/` or deleted",
        wrong.join("\n")
    );
}

/// The command for the skill `name`: its text without the `name:` line of its front matter
fn mirror(skill: &str, name: &str) -> String {
    let name_line = format!("name: {name}");
    let skill = skill.replace("\r\n", "\n");
    let mut lines: Vec<&str> = skill.split('\n').collect();
    if let Some(at) = lines.iter().position(|line| line.trim_end() == name_line) {
        lines.remove(at);
    }
    lines.join("\n")
}

#[test]
fn test_a_command_is_its_skill_without_the_name() {
    let skill = "---\r\nname: next\r\ndescription: Build it\r\n---\r\n\r\n# /next\r\n\r\nname: next stays in the text\r\n";
    let command = "---\ndescription: Build it\n---\n\n# /next\n\nname: next stays in the text\n";
    assert_eq!(mirror(skill, "next"), command);
    assert_ne!(mirror(skill, "next"), command.replace("Build it", "Build"));
}
