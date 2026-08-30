//! Doc-drift guard: every long CLI flag must appear in
//! docs/cli-reference.md. A feature nobody can discover might as well
//! not exist — and this test makes forgetting the doc a red build.

use std::collections::BTreeSet;
use std::process::Command;

#[test]
fn every_cli_flag_is_documented_in_the_reference() {
    let help = Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg("--help")
        .output()
        .unwrap();
    let help_text = String::from_utf8_lossy(&help.stdout).to_string();

    let flag_re = regex::Regex::new(r"--[a-z][a-z0-9-]+").unwrap();
    let flags: BTreeSet<String> = flag_re
        .find_iter(&help_text)
        .map(|m| m.as_str().to_string())
        .filter(|f| f != "--help" && f != "--version")
        .collect();
    assert!(
        flags.len() > 30,
        "sanity: the CLI has many flags, found {}",
        flags.len()
    );

    let reference = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/cli-reference.md"),
    )
    .expect("docs/cli-reference.md exists");

    let missing: Vec<&String> = flags
        .iter()
        .filter(|flag| !reference.contains(flag.as_str()))
        .collect();
    assert!(
        missing.is_empty(),
        "these flags are invisible to users — add them to docs/cli-reference.md:\n{missing:#?}"
    );
}

#[test]
fn specialized_views_table_matches_help() {
    let help = Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg("--help")
        .output()
        .unwrap();
    let help_text = String::from_utf8_lossy(&help.stdout).to_string();

    // The section runs from the "Specialized views:" heading to the next
    // column-0 heading. Descriptions cite other flags ("of --flag", "from
    // --graph-file"), so only option lines count: clap indents them with
    // exactly six spaces, deeper lines are wrapped description text.
    let mut in_section = false;
    let mut help_flags = BTreeSet::new();
    let flag_re = regex::Regex::new(r"^--[a-z][a-z0-9-]+").unwrap();
    for line in help_text.lines() {
        if line.trim_end() == "Specialized views:" {
            in_section = true;
            continue;
        }
        if !in_section {
            continue;
        }
        if !line.starts_with(' ') && line.trim_end().ends_with(':') {
            break;
        }
        if let Some(option) = line.strip_prefix("      --") {
            if let Some(m) = flag_re.find(&format!("--{option}")) {
                help_flags.insert(m.as_str().to_string());
            }
        }
    }
    assert!(
        help_flags.len() > 30,
        "sanity: the Specialized views group is large, found {}",
        help_flags.len()
    );

    let reference = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/cli-reference.md"),
    )
    .expect("docs/cli-reference.md exists");
    let section = reference
        .split("## The question each view answers")
        .nth(1)
        .expect("the question table section exists")
        .split("\n## ")
        .next()
        .unwrap();

    let row_re = regex::Regex::new(r"(?m)^\| `(--[a-z][a-z0-9-]+)").unwrap();
    let doc_flags: BTreeSet<String> = row_re
        .captures_iter(section)
        .map(|c| c[1].to_string())
        .collect();

    let missing: Vec<&String> = help_flags.difference(&doc_flags).collect();
    let stale: Vec<&String> = doc_flags.difference(&help_flags).collect();
    assert!(
        missing.is_empty(),
        "Specialized views flags with no row in the question table:\n{missing:#?}"
    );
    assert!(
        stale.is_empty(),
        "question-table rows for flags no longer in Specialized views:\n{stale:#?}"
    );

    // The intro states a count; keep the number honest too.
    let count_re = regex::Regex::new(r"one of the (\d+) flags grouped under").unwrap();
    let claimed: usize = count_re
        .captures(section)
        .expect("the intro sentence states the view count")[1]
        .parse()
        .unwrap();
    assert_eq!(
        claimed,
        help_flags.len(),
        "the count in the intro sentence drifted from the real group size"
    );
}
