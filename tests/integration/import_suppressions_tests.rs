//! Integration tests for --import-suppressions: teams arriving from
//! Detekt already triaged their unused-code findings — converting the
//! @Suppress("unused") annotations sprinkled through the code into a
//! SearchDeadCode baseline makes the migration a no-op instead of a
//! re-triage.

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

#[test]
fn suppressed_symbols_land_in_the_baseline() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Kept.kt"),
        concat!(
            "package sample\n\n",
            "@Suppress(\"unused\")\n",
            "class Kept {\n    fun hold() {}\n}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Ghost.kt"),
        "package sample\n\nclass Ghost {\n    fun haunt() {}\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();
    let baseline = temp.path().join("baseline.json");

    let out = bin(
        temp.path(),
        &["--import-suppressions", baseline.to_str().unwrap()],
    );
    assert!(out.status.success(), "import failed:\n{out:?}");

    let json = fs::read_to_string(&baseline).unwrap();
    assert!(
        json.contains("Kept"),
        "the suppressed class is baselined, json was:\n{json}"
    );
    assert!(
        !json.contains("Ghost"),
        "unsuppressed corpses are NOT swept in — they stay reportable, json was:\n{json}"
    );
}

#[test]
fn the_imported_baseline_silences_the_suppressed_finding() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Kept.kt"),
        concat!(
            "package sample\n\n",
            "@Suppress(\"unused\")\n",
            "class Kept {\n    fun hold() {}\n}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();
    let baseline = temp.path().join("baseline.json");
    let import = bin(
        temp.path(),
        &["--import-suppressions", baseline.to_str().unwrap()],
    );
    assert!(import.status.success());

    let report = bin(temp.path(), &["--baseline", baseline.to_str().unwrap()]);
    let stdout = stdout_of(&report);
    assert!(
        !stdout.contains("'Kept'"),
        "the migrated suppression keeps working, stdout was:\n{stdout}"
    );
}

#[test]
fn importing_into_an_existing_baseline_appends_without_duplicates() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Kept.kt"),
        concat!(
            "package sample\n\n",
            "@Suppress(\"unused\")\n",
            "class Kept {\n    fun hold() {}\n}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();
    let baseline = temp.path().join("baseline.json");

    let first = bin(
        temp.path(),
        &["--import-suppressions", baseline.to_str().unwrap()],
    );
    assert!(first.status.success());
    let second = bin(
        temp.path(),
        &["--import-suppressions", baseline.to_str().unwrap()],
    );
    assert!(second.status.success(), "re-import is safe:\n{second:?}");

    let json = fs::read_to_string(&baseline).unwrap();
    assert_eq!(
        json.matches("\"Kept\"").count(),
        1,
        "idempotent import, json was:\n{json}"
    );
}

#[test]
fn other_suppress_reasons_are_not_imported() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Noisy.kt"),
        concat!(
            "package sample\n\n",
            "@Suppress(\"MagicNumber\")\n",
            "class Noisy {\n    fun beep() {}\n}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();
    let baseline = temp.path().join("baseline.json");

    let out = bin(
        temp.path(),
        &["--import-suppressions", baseline.to_str().unwrap()],
    );
    assert!(out.status.success());
    let json = fs::read_to_string(&baseline).unwrap_or_default();
    assert!(
        !json.contains("Noisy"),
        "only unused-flavored suppressions migrate, json was:\n{json}"
    );
}
