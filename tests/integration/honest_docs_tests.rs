//! Honesty guard for DETECTORS.md: a rule documented "enabled by
//! default" must actually fire without flags, and the opt-in style
//! lints must say so. Docs promising behavior the binary does not
//! have cost users an afternoon each.

use std::fs;
use std::path::Path;
use std::process::Command;

fn detectors_md() -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("DETECTORS.md")).unwrap()
}

/// The DC014 fixture: a redundant `this.` that only --style reports.
fn style_fixture() -> tempfile::TempDir {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Account.kt"),
        concat!(
            "package sample\n\n",
            "class Account {\n",
            "    private var balance: Int = 0\n\n",
            "    fun deposit(amount: Int) {\n",
            "        this.balance = amount\n",
            "    }\n",
            "}\n\n",
            "fun main() {\n",
            "    Account().deposit(1)\n",
            "}\n",
        ),
    )
    .unwrap();
    temp
}

#[test]
fn style_lints_are_opt_in_and_the_doc_says_so() {
    let temp = style_fixture();
    let without = Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(temp.path())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&without.stdout).to_string();
    assert!(
        !stdout.contains("DC014"),
        "DC014 must NOT fire by default, stdout was:\n{stdout}"
    );

    let with_style = Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(temp.path())
        .arg("--style")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&with_style.stdout).to_string();
    assert!(
        stdout.contains("DC014"),
        "sanity: --style does fire DC014, stdout was:\n{stdout}"
    );

    // and the doc must tell that truth for every style lint
    let doc = detectors_md();
    for code in ["DC014", "DC015", "DC016"] {
        let section_start = doc.find(&format!("### {code}")).expect("section exists");
        let section = &doc[section_start..(section_start + 600).min(doc.len())];
        assert!(
            section.contains("--style"),
            "{code} is opt-in via --style and its doc must say so, section was:\n{section}"
        );
        assert!(
            !section.contains("Enabled by default"),
            "{code} must not claim default enablement, section was:\n{section}"
        );
    }
}
