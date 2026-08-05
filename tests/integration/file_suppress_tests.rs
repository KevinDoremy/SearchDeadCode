//! `@file:Suppress` as a reservation, never as a silence.
//!
//! The tool used to ignore file-level suppressions entirely: seventeen files
//! of the demo corpus carry `@file:Suppress("unused")` and every one of them
//! still received findings. The fix keeps the finding — a file-wide opt-out is
//! not evidence a symbol is alive, and hiding it is how a temporary silence
//! becomes permanent — and marks it instead.

use std::fs;
use std::path::Path;
use std::process::Output;

fn run(dir: &Path, extra: &[&str]) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .args(extra)
        .output()
        .unwrap()
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

/// Every issue as (code, confidence), so a test can compare two runs.
fn issues(dir: &Path) -> Vec<(String, String, String)> {
    let out = stdout_of(&run(dir, &["--format", "json"]));
    let start = out.find('{').unwrap_or(0);
    let parsed: serde_json::Value = serde_json::from_str(&out[start..]).unwrap();
    parsed["issues"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| {
            (
                i["code"].as_str().unwrap_or("").to_string(),
                i["confidence"].as_str().unwrap_or("").to_string(),
                i["message"].as_str().unwrap_or("").to_string(),
            )
        })
        .collect()
}

fn write_main(dir: &Path) {
    fs::write(
        dir.join("Main.kt"),
        "package s\n\nfun main() {\n    println(1)\n}\n",
    )
    .unwrap();
}

#[test]
fn a_marked_file_still_reports_its_dead_class() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Dead.kt"),
        "@file:Suppress(\"unused\")\n\npackage s\n\nclass NobodyUsesThis\n",
    )
    .unwrap();
    write_main(temp.path());

    let stdout = stdout_of(&run(temp.path(), &[]));

    assert!(
        stdout.contains("NobodyUsesThis"),
        "the finding stays in the report, stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("@file:Suppress"),
        "and says why it is reserved, stdout:\n{stdout}"
    );
}

#[test]
fn the_reservation_removes_nothing_and_lowers_confidence() {
    let marked = tempfile::tempdir().unwrap();
    let plain = tempfile::tempdir().unwrap();
    for (dir, header) in [
        (marked.path(), "@file:Suppress(\"unused\")\n\n"),
        (plain.path(), ""),
    ] {
        fs::write(
            dir.join("Dead.kt"),
            format!("{header}package s\n\nclass NobodyUsesThis\n"),
        )
        .unwrap();
        write_main(dir);
    }

    let with_mark = issues(marked.path());
    let without = issues(plain.path());

    assert_eq!(
        with_mark.len(),
        without.len(),
        "the same findings on both sides — the mark must not remove one"
    );
    let marked_conf = &with_mark.iter().find(|i| i.0 == "DC001").unwrap().1;
    let plain_conf = &without.iter().find(|i| i.0 == "DC001").unwrap().1;
    assert_ne!(
        marked_conf, plain_conf,
        "the marked one carries a lower confidence"
    );
    assert_eq!(plain_conf, "medium");
    assert_eq!(marked_conf, "low");
}

#[test]
fn an_unrelated_suppression_changes_nothing() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Dead.kt"),
        "@file:Suppress(\"MagicNumber\")\n\npackage s\n\nclass NobodyUsesThis\n",
    )
    .unwrap();
    write_main(temp.path());

    let stdout = stdout_of(&run(temp.path(), &[]));

    assert!(stdout.contains("NobodyUsesThis"));
    assert!(
        !stdout.contains("@file:Suppress"),
        "MagicNumber says nothing about reachability, stdout:\n{stdout}"
    );
}

#[test]
fn a_parameter_suppression_does_not_reserve_a_declaration() {
    // The word-boundary rule earning its keep: `UNUSED_PARAMETER` contains
    // `unused`, but a file silencing the parameter warning has said nothing
    // about whether its classes are reachable.
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Both.kt"),
        concat!(
            "@file:Suppress(\"UNUSED_PARAMETER\")\n\n",
            "package s\n\n",
            "class NobodyUsesThis\n\n",
            "fun used(a: Int, neverRead: String): Int = a\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package s\n\nfun main() {\n    println(used(1, \"x\"))\n}\n",
    )
    .unwrap();

    let all = issues(temp.path());
    let dc001 = all.iter().find(|i| i.0 == "DC001");
    let dc003 = all.iter().find(|i| i.0 == "DC003");

    assert!(
        dc003.is_none() || dc003.is_some_and(|i| i.2.contains("@file:Suppress")),
        "DC003 is what the file declined: {dc003:?}"
    );
    assert!(
        dc001.is_some_and(|i| !i.2.contains("@file:Suppress")),
        "DC001 was never declined: {dc001:?}"
    );
}

#[test]
fn a_rule_code_can_be_named_directly() {
    // Every rule answers to its own code, even the ones with no Kotlin or
    // Detekt counterpart to borrow a name from.
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Dead.kt"),
        "@file:Suppress(\"DC001\")\n\npackage s\n\nclass NobodyUsesThis\n",
    )
    .unwrap();
    write_main(temp.path());

    let stdout = stdout_of(&run(temp.path(), &[]));

    assert!(
        stdout.contains("NobodyUsesThis") && stdout.contains("DC001\""),
        "the code names the rule it silences, stdout:\n{stdout}"
    );
}

#[test]
fn the_corpus_form_with_a_trailing_comment_is_read() {
    // The exact shape found seventeen times in the demo corpus.
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Dead.kt"),
        "@file:Suppress(\"unused\") // demo fixture: showcases another feature\n\npackage s\n\nclass NobodyUsesThis\n",
    )
    .unwrap();
    write_main(temp.path());

    assert!(
        stdout_of(&run(temp.path(), &[])).contains("@file:Suppress"),
        "a trailing comment must not hide the annotation"
    );
}

#[test]
fn the_bracketed_form_is_read() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Dead.kt"),
        "@file:[JvmName(\"Helpers\") Suppress(\"unused\")]\n\npackage s\n\nclass NobodyUsesThis\n",
    )
    .unwrap();
    write_main(temp.path());

    assert!(stdout_of(&run(temp.path(), &[])).contains("@file:Suppress"));
}

#[test]
fn a_suppression_quoted_inside_the_body_does_not_count() {
    // The header is cut at `package`, so a string literal further down cannot
    // silence the file.
    let temp = tempfile::tempdir().unwrap();
    let filler = "// padding\n".repeat(30);
    fs::write(
        temp.path().join("Dead.kt"),
        format!("package s\n\n{filler}val quoted = \"@file:Suppress(\\\"unused\\\")\"\n\nclass NobodyUsesThis\n"),
    )
    .unwrap();
    write_main(temp.path());

    let stdout = stdout_of(&run(temp.path(), &[]));

    assert!(stdout.contains("NobodyUsesThis"));
    assert!(
        !stdout.contains("[file marked"),
        "a quoted annotation is not an annotation, stdout:\n{stdout}"
    );
}

#[test]
fn a_java_file_is_never_touched() {
    // `@file:` is Kotlin syntax; a Java comment carrying the same text must
    // not reserve anything.
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Dead.java"),
        "// @file:Suppress(\"unused\")\npackage app;\n\npublic class NobodyUsesThis {}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.java"),
        "package app;\n\npublic class Main {\n    public static void main(String[] a) {}\n}\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path(), &[]));

    assert!(
        !stdout.contains("[file marked"),
        "Java has no file annotations, stdout:\n{stdout}"
    );
}

#[test]
fn an_inline_ignore_still_wins_over_the_reservation() {
    // `// deadcode:ignore(reason)` is applied before the reservation pass and
    // removes the finding outright — the two must not both fire.
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Dead.kt"),
        concat!(
            "@file:Suppress(\"unused\")\n\n",
            "package s\n\n",
            "// deadcode:ignore(kept on purpose for the demo)\n",
            "class NobodyUsesThis\n",
        ),
    )
    .unwrap();
    write_main(temp.path());

    let stdout = stdout_of(&run(temp.path(), &[]));

    // The name still shows in the "ignored inline" tally, which is the point
    // of that tally; what must not happen is a finding, reserved or otherwise.
    assert!(
        stdout.contains("No dead code found"),
        "the inline directive removes it, stdout:\n{stdout}"
    );
    assert!(
        !stdout.contains("[file marked"),
        "and the two mechanisms do not both fire, stdout:\n{stdout}"
    );
}

#[test]
fn a_commented_out_suppression_is_not_a_suppression() {
    // `// TODO remove: @file:Suppress("unused")` is a note ABOUT an opt-out,
    // not an opt-out. The header was read raw, so the reservation named a
    // suppression that was not in effect and lowered the confidence of a
    // finding nothing had declined. Found by running the two analyzers over
    // identical fixtures.
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Dead.kt"),
        "// TODO remove: @file:Suppress(\"unused\")\npackage s\n\nclass NobodyUsesThis\n",
    )
    .unwrap();
    write_main(temp.path());

    let stdout = stdout_of(&run(temp.path(), &[]));

    assert!(stdout.contains("NobodyUsesThis"));
    assert!(
        !stdout.contains("[file marked"),
        "no annotation is in effect here, stdout:\n{stdout}"
    );
}

#[test]
fn a_licence_block_mentioning_the_annotation_changes_nothing() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Dead.kt"),
        concat!(
            "/*\n",
            " * Licence. Do not write @file:Suppress(\"unused\") here.\n",
            " */\n",
            "package s\n\n",
            "class NobodyUsesThis\n",
        ),
    )
    .unwrap();
    write_main(temp.path());

    let stdout = stdout_of(&run(temp.path(), &[]));

    assert!(stdout.contains("NobodyUsesThis"));
    assert!(
        !stdout.contains("[file marked"),
        "a block comment is not an annotation, stdout:\n{stdout}"
    );
}
