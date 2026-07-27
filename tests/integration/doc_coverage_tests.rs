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
