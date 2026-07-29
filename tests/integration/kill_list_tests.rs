//! Integration tests for --kill-list: "if I delete X, what else falls?"

use std::fs;
use std::path::Path;
use std::process::Output;

/// Main uses SharedFormatter. OldReportScreen (dead) uses OldReportPrinter
/// (its exclusive dependent) and SharedFormatter (shared with live code).
fn write_sample_project(dir: &Path) {
    fs::write(
        dir.join("Main.kt"),
        "package sample\n\nfun main() {\n    SharedFormatter().format()\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("SharedFormatter.kt"),
        "package sample\n\nclass SharedFormatter {\n    fun format() {}\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("OldReportScreen.kt"),
        "package sample\n\nclass OldReportScreen {\n    fun show() {\n        OldReportPrinter().print()\n        SharedFormatter().format()\n    }\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("OldReportPrinter.kt"),
        "package sample\n\nclass OldReportPrinter {\n    fun print() {}\n}\n",
    )
    .unwrap();
}

fn run_kill_list(dir: &Path, symbol: &str) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .args(["--kill-list", symbol])
        .output()
        .unwrap()
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn kill_list_includes_the_target_and_its_exclusive_dependents() {
    let temp = tempfile::tempdir().unwrap();
    write_sample_project(temp.path());

    let output = run_kill_list(temp.path(), "OldReportScreen");

    let stdout = stdout_of(&output);
    assert!(
        stdout.contains("OldReportScreen"),
        "the target itself is in the kill-list, stdout was:\n{stdout}"
    );
    assert!(
        stdout.contains("OldReportPrinter"),
        "a class only used by the target falls with it, stdout was:\n{stdout}"
    );
}

#[test]
fn kill_list_spares_symbols_shared_with_live_code() {
    let temp = tempfile::tempdir().unwrap();
    write_sample_project(temp.path());

    let output = run_kill_list(temp.path(), "OldReportScreen");

    let stdout = stdout_of(&output);
    assert!(
        !stdout.contains("SharedFormatter"),
        "a class also used by live code must not be in the kill-list, stdout was:\n{stdout}"
    );
}

#[test]
fn kill_list_reports_size() {
    let temp = tempfile::tempdir().unwrap();
    write_sample_project(temp.path());

    let output = run_kill_list(temp.path(), "OldReportScreen");

    let stdout = stdout_of(&output);
    assert!(
        stdout.contains("lines"),
        "the kill-list reports its estimated size in lines, stdout was:\n{stdout}"
    );
}

#[test]
fn kill_list_for_unknown_symbol() {
    let temp = tempfile::tempdir().unwrap();
    write_sample_project(temp.path());

    let output = run_kill_list(temp.path(), "NoSuchThing");

    let stdout = stdout_of(&output);
    assert!(
        stdout.contains("not found"),
        "an unknown symbol yields a clear message, stdout was:\n{stdout}"
    );
}

#[test]
fn an_ambiguous_homonym_in_another_class_does_not_fall_with_the_target() {
    // Cas réel : la kill-list d'une activity embarquait des `onStart` de
    // modules sans aucun lien — la fermeture avant suivait les arêtes
    // ambiguës (résolution par nom simple : un appel `x.refresh()` lie
    // TOUS les `refresh` du repo). Une devinette par homonymie ne
    // condamne pas un symbole d'un autre module.
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    Doomed().go()\n    Survivor().keepAlive()\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Doomed.kt"),
        "package sample\n\nclass Doomed {\n    fun go() {\n        Buddy().refresh()\n    }\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Buddy.kt"),
        "package sample\n\nclass Buddy {\n    fun refresh() {}\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Survivor.kt"),
        concat!(
            "package sample\n\n",
            "class Survivor {\n",
            "    fun keepAlive() {}\n\n",
            "    fun refresh() {}\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run_kill_list(temp.path(), "Doomed"));
    assert!(
        !stdout.contains("Survivor.kt"),
        "l'homonyme `refresh` d'une classe vivante ne tombe pas avec Doomed, stdout:\n{stdout}"
    );
}
