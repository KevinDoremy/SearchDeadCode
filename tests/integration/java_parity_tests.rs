//! Integration tests for Java parity: write-only prefs and write-only
//! DAO detection only scanned Kotlin files, while a typical mixed repo
//! is half Java — half the codebase escaped the advanced detectors.

use std::fs;
use std::path::Path;
use std::process::Output;

fn run(dir: &Path) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .output()
        .unwrap()
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn write_main(dir: &Path) {
    fs::write(
        dir.join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();
}

#[test]
fn a_java_write_only_pref_is_reported() {
    let temp = tempfile::tempdir().unwrap();
    write_main(temp.path());
    fs::write(
        temp.path().join("JavaPrefs.java"),
        concat!(
            "package sample;\n\n",
            "public class JavaPrefs {\n",
            "    void save(Editor editor) {\n",
            "        editor.putString(\"java_dead_pref\", \"x\");\n",
            "        editor.putString(\"java_live_pref\", \"y\");\n",
            "    }\n\n",
            "    String load(Prefs prefs) {\n",
            "        return prefs.getString(\"java_live_pref\", null);\n",
            "    }\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("java_dead_pref"),
        "a Java key written but never read is a finding, stdout was:\n{stdout}"
    );
    assert!(
        !stdout.contains("\"java_live_pref\" is written but never read")
            && !stdout.contains("java_live_pref\" is written"),
        "a read key is not write-only, stdout was:\n{stdout}"
    );
}

#[test]
fn a_java_write_only_dao_is_reported() {
    let temp = tempfile::tempdir().unwrap();
    write_main(temp.path());
    fs::write(
        temp.path().join("JavaLogDao.java"),
        concat!(
            "package sample;\n\n",
            "@Dao\n",
            "public interface JavaLogDao {\n",
            "    @Insert\n",
            "    void insert(String entry);\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("JavaLogDao"),
        "a Java DAO with @Insert and no @Query is a finding, stdout was:\n{stdout}"
    );
}

#[test]
fn a_java_dao_with_queries_stays_silent() {
    let temp = tempfile::tempdir().unwrap();
    write_main(temp.path());
    fs::write(
        temp.path().join("HealthyDao.java"),
        concat!(
            "package sample;\n\n",
            "@Dao\n",
            "public interface HealthyDao {\n",
            "    @Insert\n",
            "    void insert(String entry);\n\n",
            "    @Query(\"SELECT * FROM log\")\n",
            "    java.util.List<String> all();\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("HealthyDao' has @Insert"),
        "reads exist, nothing write-only, stdout was:\n{stdout}"
    );
}
