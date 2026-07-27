//! Integration tests: intent extras, write-only prefs and write-only
//! DAOs must reach JSON like any other finding (second half of the
//! out-of-graph work — DC019, DC010, DC011).

use std::fs;
use std::path::Path;
use std::process::Output;

fn write_project(dir: &Path) {
    fs::write(
        dir.join("Sender.kt"),
        concat!(
            "package sample\n\n",
            "fun send(intent: Intent) {\n",
            "    intent.putExtra(\"ghost_key\", 1)\n",
            "    intent.putExtra(\"seen_key\", 2)\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        dir.join("Receiver.kt"),
        concat!(
            "package sample\n\n",
            "fun receive(intent: Intent) {\n",
            "    val v = intent.getIntExtra(\"seen_key\", 0)\n",
            "    println(v)\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        dir.join("Prefs.kt"),
        concat!(
            "package sample\n\n",
            "fun save(editor: Editor) {\n",
            "    editor.putString(\"dead_pref\", \"x\")\n",
            "    editor.putString(\"live_pref\", \"y\")\n",
            "}\n\n",
            "fun load(prefs: Prefs): String? {\n",
            "    return prefs.getString(\"live_pref\", null)\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        dir.join("LogDao.kt"),
        concat!(
            "package sample\n\n",
            "@Dao\n",
            "interface LogDao {\n",
            "    @Insert\n",
            "    fun insert(entry: String)\n",
            "}\n",
        ),
    )
    .unwrap();
}

fn run(dir: &Path, extra: &[&str]) -> Output {
    // extras/prefs/DAO detection is on by default
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .args(extra)
        .output()
        .unwrap()
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn an_unretrieved_extra_reaches_json() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let stdout = stdout_of(&run(temp.path(), &["--format", "json"]));
    assert!(
        stdout.contains("ghost_key") && stdout.contains("DC019"),
        "putExtra with no get is a JSON finding, stdout was:\n{stdout}"
    );
    assert!(
        !stdout.contains("\"seen_key\""),
        "a retrieved extra is not a finding, stdout was:\n{stdout}"
    );
}

#[test]
fn a_write_only_pref_reaches_json() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let stdout = stdout_of(&run(temp.path(), &["--format", "json"]));
    assert!(
        stdout.contains("dead_pref") && stdout.contains("DC010"),
        "a key written but never read is a JSON finding, stdout was:\n{stdout}"
    );
    assert!(
        !stdout.contains("\"live_pref\""),
        "a read key is not a finding, stdout was:\n{stdout}"
    );
}

#[test]
fn a_write_only_dao_reaches_json() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let stdout = stdout_of(&run(temp.path(), &["--format", "json"]));
    assert!(
        stdout.contains("LogDao") && stdout.contains("DC011"),
        "a DAO with @Insert and no @Query is a JSON finding, stdout was:\n{stdout}"
    );
}

#[test]
fn terminal_mode_still_reports_all_three() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let stdout = stdout_of(&run(temp.path(), &[]));
    assert!(
        stdout.contains("ghost_key") && stdout.contains("dead_pref") && stdout.contains("LogDao"),
        "terminal keeps all three findings, stdout was:\n{stdout}"
    );
}
