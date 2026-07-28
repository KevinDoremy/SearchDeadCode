//! Integration tests for --import-detekt-baseline: the other half of
//! the Detekt migration. --import-suppressions converts @Suppress
//! annotations in the code; this converts the detekt-baseline.xml the
//! team already triaged — same no-op migration, different source.

use std::fs;
use std::path::Path;
use std::process::Output;

fn bin(dir: &Path, args: &[&str]) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .args(args)
        .output()
        .unwrap()
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn detekt_xml(ids: &[&str]) -> String {
    let entries: String = ids
        .iter()
        .map(|id| format!("    <ID>{id}</ID>\n"))
        .collect();
    format!(
        "<?xml version=\"1.0\" ?>\n<SmellBaseline>\n  <ManuallySuppressedIssues/>\n  <CurrentIssues>\n{entries}  </CurrentIssues>\n</SmellBaseline>\n"
    )
}

#[test]
fn unused_rule_entries_land_in_the_baseline() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Kept.kt"),
        "package sample\n\nclass Kept {\n    fun hold() {}\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();
    let xml = temp.path().join("detekt-baseline.xml");
    fs::write(&xml, detekt_xml(&["UnusedPrivateClass:Kept.kt$class Kept"])).unwrap();
    let baseline = temp.path().join("baseline.json");

    let out = bin(
        temp.path(),
        &[
            "--import-detekt-baseline",
            xml.to_str().unwrap(),
            "--baseline",
            baseline.to_str().unwrap(),
        ],
    );
    assert!(out.status.success(), "import failed:\n{out:?}");

    let json = fs::read_to_string(&baseline).unwrap();
    assert!(
        json.contains("Kept"),
        "the triaged class is baselined, json was:\n{json}"
    );
}

#[test]
fn non_unused_rules_are_not_imported() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Noisy.kt"),
        "package sample\n\nclass Noisy {\n    fun beep() {}\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();
    let xml = temp.path().join("detekt-baseline.xml");
    fs::write(&xml, detekt_xml(&["MagicNumber:Noisy.kt$42"])).unwrap();
    let baseline = temp.path().join("baseline.json");

    let out = bin(
        temp.path(),
        &[
            "--import-detekt-baseline",
            xml.to_str().unwrap(),
            "--baseline",
            baseline.to_str().unwrap(),
        ],
    );
    assert!(out.status.success());
    let json = fs::read_to_string(&baseline).unwrap_or_default();
    assert!(
        !json.contains("Noisy"),
        "MagicNumber is Detekt's problem, not ours, json was:\n{json}"
    );
}

#[test]
fn entries_pointing_at_nothing_are_skipped_and_counted() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();
    let xml = temp.path().join("detekt-baseline.xml");
    fs::write(
        &xml,
        detekt_xml(&["UnusedPrivateClass:Vanished.kt$class Vanished"]),
    )
    .unwrap();
    let baseline = temp.path().join("baseline.json");

    let out = bin(
        temp.path(),
        &[
            "--import-detekt-baseline",
            xml.to_str().unwrap(),
            "--baseline",
            baseline.to_str().unwrap(),
        ],
    );
    assert!(out.status.success(), "a stale entry is not fatal:\n{out:?}");
    let stdout = stdout_of(&out);
    assert!(
        stdout.contains("skipped") || stdout.contains("1 unresolved"),
        "the unresolved entry is reported, not silently dropped, stdout was:\n{stdout}"
    );
}

#[test]
fn the_imported_baseline_silences_the_finding() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Kept.kt"),
        "package sample\n\nclass Kept {\n    fun hold() {}\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();
    let xml = temp.path().join("detekt-baseline.xml");
    fs::write(&xml, detekt_xml(&["UnusedPrivateClass:Kept.kt$class Kept"])).unwrap();
    let baseline = temp.path().join("baseline.json");
    let import = bin(
        temp.path(),
        &[
            "--import-detekt-baseline",
            xml.to_str().unwrap(),
            "--baseline",
            baseline.to_str().unwrap(),
        ],
    );
    assert!(import.status.success());

    let report = bin(temp.path(), &["--baseline", baseline.to_str().unwrap()]);
    let stdout = stdout_of(&report);
    assert!(
        !stdout.contains("'Kept'"),
        "the migrated triage keeps working, stdout was:\n{stdout}"
    );
}

#[test]
fn reimporting_is_idempotent() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Kept.kt"),
        "package sample\n\nclass Kept {\n    fun hold() {}\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();
    let xml = temp.path().join("detekt-baseline.xml");
    fs::write(&xml, detekt_xml(&["UnusedPrivateClass:Kept.kt$class Kept"])).unwrap();
    let baseline = temp.path().join("baseline.json");
    let args = [
        "--import-detekt-baseline",
        xml.to_str().unwrap(),
        "--baseline",
        baseline.to_str().unwrap(),
    ];

    assert!(bin(temp.path(), &args).status.success());
    assert!(bin(temp.path(), &args).status.success());

    let json = fs::read_to_string(&baseline).unwrap();
    assert_eq!(
        json.matches("\"Kept\"").count(),
        1,
        "idempotent import, json was:\n{json}"
    );
}

#[test]
fn a_missing_xml_is_a_clear_error() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();
    let baseline = temp.path().join("baseline.json");

    let out = bin(
        temp.path(),
        &[
            "--import-detekt-baseline",
            temp.path().join("nope.xml").to_str().unwrap(),
            "--baseline",
            baseline.to_str().unwrap(),
        ],
    );
    assert!(!out.status.success(), "a missing file cannot succeed");
}

#[test]
fn manually_suppressed_issues_are_imported_too() {
    // Detekt splits its baseline into CurrentIssues and
    // ManuallySuppressedIssues — a triage lives in BOTH sections, and a
    // future switch to a section-aware XML parser must not drop one
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Kept.kt"),
        "package sample\n\nclass Kept {\n    fun hold() {}\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();
    let xml = temp.path().join("detekt-baseline.xml");
    fs::write(
        &xml,
        concat!(
            "<?xml version=\"1.0\" ?>\n<SmellBaseline>\n",
            "  <ManuallySuppressedIssues>\n",
            "    <ID>UnusedPrivateClass:Kept.kt$class Kept</ID>\n",
            "  </ManuallySuppressedIssues>\n",
            "  <CurrentIssues/>\n",
            "</SmellBaseline>\n",
        ),
    )
    .unwrap();
    let baseline = temp.path().join("baseline.json");

    let out = bin(
        temp.path(),
        &[
            "--import-detekt-baseline",
            xml.to_str().unwrap(),
            "--baseline",
            baseline.to_str().unwrap(),
        ],
    );
    assert!(out.status.success(), "import failed:\n{out:?}");
    let json = fs::read_to_string(&baseline).unwrap_or_default();
    assert!(
        json.contains("Kept"),
        "a manually suppressed triage is still a triage, json was:\n{json}"
    );
}
