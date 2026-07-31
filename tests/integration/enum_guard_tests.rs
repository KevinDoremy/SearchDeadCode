//! Integration tests for the DC005 iteration and test-file guards.
//!
//! An enum iterated exhaustively reaches every case, with or without the
//! type prefix; an enum declared in a test source set is the test's
//! business. Neither may produce unused-case findings, and a genuinely
//! dead case must keep being reported.

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

#[test]
fn companion_values_iteration_protects_every_case() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Status.kt"),
        concat!(
            "package sample\n\n",
            "enum class Status {\n",
            "    ACTIVE, PAUSED, RETIRED;\n\n",
            "    companion object {\n",
            "        fun from(name: String): Status? = values().firstOrNull { it.name == name }\n",
            "    }\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(Status.from(\"ACTIVE\"))\n}\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("DC005"),
        "a bare values() in the companion reaches every case, stdout was:\n{stdout}"
    );
}

#[test]
fn enum_entries_helper_protects_every_case() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Mode.kt"),
        concat!(
            "package sample\n\n",
            "enum class Mode { DAY, NIGHT }\n\n",
            "fun allModes(): List<Mode> = enumEntries<Mode>()\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(allModes())\n}\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("DC005"),
        "enumEntries<Mode>() iterates every case, stdout was:\n{stdout}"
    );
}

#[test]
fn qualified_references_keep_cases_alive() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Level.kt"),
        concat!(
            "package sample\n\n",
            "enum class Level { LOW, HIGH }\n\n",
            "fun pick(): Level = Level.LOW\n",
            "fun other(): Level = Level.HIGH\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(pick())\n    println(other())\n}\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("DC005"),
        "Level.LOW and Level.HIGH are spelled out, stdout was:\n{stdout}"
    );
}

#[test]
fn a_test_declared_enum_is_the_tests_business() {
    let temp = tempfile::tempdir().unwrap();
    let test_dir = temp.path().join("src/test/kotlin");
    fs::create_dir_all(&test_dir).unwrap();
    fs::write(
        test_dir.join("Fixture.kt"),
        concat!(
            "package sample\n\n",
            "enum class TestFlavor { MOCKED, REAL }\n\n",
            "fun pickFlavor(): TestFlavor = TestFlavor.MOCKED\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"app\")\n}\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("DC005"),
        "enum cases declared under src/test never yield DC005, stdout was:\n{stdout}"
    );
}

#[test]
fn a_truly_dead_case_is_still_reported() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Color.kt"),
        concat!(
            "package sample\n\n",
            "enum class Color { USED, GHOST }\n\n",
            "fun paint(): Color = Color.USED\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(paint())\n}\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("DC005") && stdout.contains("GHOST"),
        "the guards must not silence a genuinely dead case, stdout was:\n{stdout}"
    );
}
