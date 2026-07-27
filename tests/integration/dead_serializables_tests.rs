//! Integration tests for --dead-serializables: a @Serializable class
//! with zero incoming references survives only through its annotation.
//! kotlinx.serialization needs a static reference to (de)serialize, so
//! an unreferenced DTO is a corpse the blanket retention hides.

use std::fs;
use std::path::Path;
use std::process::Output;

fn run(dir: &Path) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .arg("--dead-serializables")
        .output()
        .unwrap()
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn an_unreferenced_serializable_is_listed() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("GhostDto.kt"),
        concat!(
            "package sample\n\n",
            "@Serializable\n",
            "class GhostDto(val payload: String)\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("GhostDto"),
        "nothing references the DTO — only the annotation keeps it, stdout was:\n{stdout}"
    );
}

#[test]
fn a_referenced_serializable_is_not_listed() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("PayloadDto.kt"),
        concat!(
            "package sample\n\n",
            "@Serializable\n",
            "class PayloadDto(val value: Int)\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        concat!(
            "package sample\n\n",
            "fun main() {\n",
            "    val json = Json.encodeToString(PayloadDto(1))\n",
            "    println(json)\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("PayloadDto"),
        "an encoded DTO is alive, stdout was:\n{stdout}"
    );
}

#[test]
fn a_nested_dto_referenced_as_a_field_type_is_not_listed() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Dtos.kt"),
        concat!(
            "package sample\n\n",
            "@Serializable\n",
            "class InnerDto(val leaf: String)\n\n",
            "@Serializable\n",
            "class OuterDto(val inner: InnerDto)\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        concat!(
            "package sample\n\n",
            "fun main() {\n",
            "    val json = Json.encodeToString(OuterDto(InnerDto(\"x\")))\n",
            "    println(json)\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("InnerDto"),
        "a field-type reference keeps the nested DTO, stdout was:\n{stdout}"
    );
}

#[test]
fn no_serializables_is_a_clean_answer() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();

    let output = run(temp.path());
    let stdout = stdout_of(&output);
    assert!(
        output.status.success(),
        "no serializables is fine, output was:\n{output:?}"
    );
    assert!(
        stdout.to_lowercase().contains("no dead serializable"),
        "the verdict is explicit, stdout was:\n{stdout}"
    );
}
