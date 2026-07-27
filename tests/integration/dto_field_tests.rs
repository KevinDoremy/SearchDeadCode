//! Integration tests for dead DTO fields: Gson writes fields through
//! reflection, so a @SerializedName property nobody ever READS is
//! business deadness the reachability graph cannot see.

use std::fs;
use std::path::Path;
use std::process::Output;

fn write_project(dir: &Path) {
    fs::write(
        dir.join("UserDto.kt"),
        concat!(
            "package sample\n\n",
            "data class UserDto(\n",
            "    @SerializedName(\"user_name\")\n",
            "    val userName: String,\n",
            "    @SerializedName(\"legacy_score\")\n",
            "    val legacyScore: Int,\n",
            ")\n",
        ),
    )
    .unwrap();
    fs::write(
        dir.join("Main.kt"),
        concat!(
            "package sample\n\n",
            "fun main(dto: UserDto) {\n",
            "    println(dto.userName)\n",
            "}\n",
        ),
    )
    .unwrap();
}

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

#[test]
fn a_deserialized_field_nobody_reads_is_reported() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let stdout = stdout_of(&run(temp.path(), &[]));
    assert!(
        stdout.contains("legacyScore"),
        "written by reflection, read by nobody, stdout was:\n{stdout}"
    );
}

#[test]
fn a_field_the_code_reads_stays_silent() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let stdout = stdout_of(&run(temp.path(), &[]));
    assert!(
        !stdout.contains("userName"),
        "dto.userName is a read, stdout was:\n{stdout}"
    );
}

#[test]
fn dead_dto_fields_reach_json_as_dc021() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let stdout = stdout_of(&run(temp.path(), &["--format", "json"]));
    assert!(
        stdout.contains("legacyScore") && stdout.contains("DC021"),
        "business deadness is CI-visible, stdout was:\n{stdout}"
    );
}

#[test]
fn unannotated_properties_are_not_this_detectors_business() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Plain.kt"),
        concat!(
            "package sample\n\n",
            "class PlainHolder {\n",
            "    val unreadValue: Int = 0\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(PlainHolder())\n}\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path(), &["--format", "json"]));
    assert!(
        !stdout.contains("DC021"),
        "no serialization marker, other detectors own this, stdout was:\n{stdout}"
    );
}
