//! Integration tests for DC005 (unused enum case) and DC007 (dead branch).
//!
//! Both were documented in DETECTORS.md but returned nothing. DC005
//! flags enum entries nobody references, unless the enum is iterated
//! reflectively (values()/entries/valueOf). DC007 flags branches gated on
//! a literal `false`, the only kind of deadness provable from text alone.

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
fn an_enum_case_nobody_references_is_reported() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Status.kt"),
        concat!(
            "package sample\n\n",
            "enum class Status {\n",
            "    ACTIVE,\n",
            "    LEGACY\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        concat!(
            "package sample\n\n",
            "fun main() {\n",
            "    val s = Status.ACTIVE\n",
            "    println(s)\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("LEGACY"),
        "LEGACY has no reference anywhere, stdout was:\n{stdout}"
    );
    assert!(
        !stdout.contains("'ACTIVE'"),
        "ACTIVE is referenced, stdout was:\n{stdout}"
    );
}

#[test]
fn an_enum_iterated_with_values_keeps_all_cases() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Status.kt"),
        concat!(
            "package sample\n\n",
            "enum class Mode {\n",
            "    FAST,\n",
            "    SLOW\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        concat!(
            "package sample\n\n",
            "fun main() {\n",
            "    for (m in Mode.values()) println(m)\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("SLOW"),
        "values() reaches every case reflectively, stdout was:\n{stdout}"
    );
}

#[test]
fn a_case_referenced_only_in_a_when_stays_alive() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Status.kt"),
        concat!(
            "package sample\n\n",
            "enum class Level {\n",
            "    LOW,\n",
            "    HIGH\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        concat!(
            "package sample\n\n",
            "fun label(level: Level): String = when (level) {\n",
            "    Level.LOW -> \"low\"\n",
            "    Level.HIGH -> \"high\"\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("DC005"),
        "a when arm is a real reference, stdout was:\n{stdout}"
    );
}

#[test]
fn a_branch_behind_literal_false_is_reported() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        concat!(
            "package sample\n\n",
            "fun main() {\n",
            "    if (false) {\n",
            "        println(\"never\")\n",
            "    }\n",
            "    println(\"always\")\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("DC007") && stdout.contains("can never execute"),
        "if (false) is provably dead, stdout was:\n{stdout}"
    );
}

#[test]
fn a_runtime_condition_is_not_a_dead_branch() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        concat!(
            "package sample\n\n",
            "fun main(debug: Boolean) {\n",
            "    if (debug) {\n",
            "        println(\"maybe\")\n",
            "    }\n",
            "    val falseAlarm = listOf(false)\n",
            "    println(falseAlarm)\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("DC007"),
        "runtime conditions and false literals outside conditions are not deadness, stdout was:\n{stdout}"
    );
}

#[test]
fn a_serialized_name_enum_case_is_never_flagged() {
    // Cas réel : enums de contrat API générés (OpenAPI/Gson) — chaque
    // entry porte @SerializedName et peut être instanciée par le JSON
    // serveur. Le détecteur skippait déjà les cases annotées, mais le
    // parser perdait les annotations des enum entries.
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(
        temp.path().join("ShapeEnum.kt"),
        concat!(
            "package sample\n\n",
            "import com.google.gson.annotations.SerializedName\n\n",
            "enum class ShapeEnum(val value: String) {\n\n",
            "    @SerializedName(value = \"circle\")\n",
            "    CIRCLE(\"circle\"),\n\n",
            "    @SerializedName(value = \"square\")\n",
            "    SQUARE(\"square\");\n",
            "}\n",
        ),
    )
    .unwrap();
    std::fs::write(
        temp.path().join("Main.kt"),
        concat!(
            "package sample\n\n",
            "fun main() {\n",
            "    println(load().value)\n",
            "}\n\n",
            "fun load(): ShapeEnum = ShapeEnum.SQUARE\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("CIRCLE"),
        "une entry @SerializedName est instanciable par désérialisation, stdout:\n{stdout}"
    );
}
