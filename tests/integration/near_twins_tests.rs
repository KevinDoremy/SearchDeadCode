//! Integration tests for --near-twins: same-named functions in
//! different files whose bodies are near-identical — the copy-paste a
//! v1→v2 migration leaves behind. Once v1 dies, this shows what stayed
//! duplicated in v2.

use std::fs;
use std::path::Path;
use std::process::Output;

fn run(dir: &Path) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .arg("--near-twins")
        .output()
        .unwrap()
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

const BODY: &str = concat!(
    "    val cleaned = input.trim()\n",
    "    if (cleaned.isEmpty()) {\n",
    "        return null\n",
    "    }\n",
    "    return cleaned.lowercase()\n",
);

#[test]
fn near_identical_bodies_across_files_are_paired() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("V1Parser.kt"),
        format!("package sample.v1\n\nfun normalize(input: String): String? {{\n{BODY}}}\n"),
    )
    .unwrap();
    fs::write(
        temp.path().join("V2Parser.kt"),
        format!("package sample.v2\n\nfun normalize(input: String): String? {{\n{BODY}}}\n"),
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("normalize"),
        "the copied pair is named, stdout was:\n{stdout}"
    );
    assert!(
        stdout.contains("V1Parser") && stdout.contains("V2Parser"),
        "both sides of the pair are located, stdout was:\n{stdout}"
    );
}

#[test]
fn same_name_different_logic_is_not_a_twin() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("MathA.kt"),
        concat!(
            "package sample.a\n\n",
            "fun compute(x: Int): Int {\n",
            "    return x + 1\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("MathB.kt"),
        concat!(
            "package sample.b\n\n",
            "fun compute(x: Int): Int {\n",
            "    val doubled = x * 2\n",
            "    val shifted = doubled - 7\n",
            "    return shifted * shifted\n",
            "}\n",
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
        !stdout.contains("compute"),
        "a shared name with different logic is fine, stdout was:\n{stdout}"
    );
}

#[test]
fn twins_inside_the_same_file_are_out_of_scope() {
    // overloads and local helpers legitimately repeat inside one file
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Overloads.kt"),
        format!(
            "package sample\n\nfun clean(input: String): String? {{\n{BODY}}}\n\nobject Wrapper {{\n    fun clean(input: String): String? {{\n{BODY}    }}\n}}\n"
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
        !stdout.contains("clean"),
        "same-file repetition is not migration debris, stdout was:\n{stdout}"
    );
}

#[test]
fn renamed_variables_do_not_hide_a_twin() {
    // type-2 clone: same structure and same calls, different local names
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("V1Cleaner.kt"),
        concat!(
            "package sample.v1\n\n",
            "fun sanitize(input: String): String? {\n",
            "    val cleaned = input.trim()\n",
            "    if (cleaned.isEmpty()) {\n",
            "        return null\n",
            "    }\n",
            "    return cleaned.lowercase()\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("V2Cleaner.kt"),
        concat!(
            "package sample.v2\n\n",
            "fun sanitize(raw: String): String? {\n",
            "    val stripped = raw.trim()\n",
            "    if (stripped.isEmpty()) {\n",
            "        return null\n",
            "    }\n",
            "    return stripped.lowercase()\n",
            "}\n",
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
        stdout.contains("sanitize"),
        "a rename is not a rewrite — the twin survives it, stdout was:\n{stdout}"
    );
}

#[test]
fn same_shape_but_different_calls_are_not_twins() {
    // identifier abstraction must NOT erase what the code actually does
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("TextA.kt"),
        concat!(
            "package sample.a\n\n",
            "fun shape(input: String): String {\n",
            "    val a = input.trim()\n",
            "    val b = a.uppercase()\n",
            "    return b.reversed()\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("TextB.kt"),
        concat!(
            "package sample.b\n\n",
            "fun shape(input: String): String {\n",
            "    val a = input.padEnd(8)\n",
            "    val b = a.lowercase()\n",
            "    return b.repeat(2)\n",
            "}\n",
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
        !stdout.contains("shape"),
        "different calls mean different behavior, stdout was:\n{stdout}"
    );
}

#[test]
fn no_twins_is_a_clean_answer() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();

    let output = run(temp.path());
    let stdout = stdout_of(&output);
    assert!(output.status.success());
    assert!(
        stdout.to_lowercase().contains("no near twins"),
        "the verdict is explicit, stdout was:\n{stdout}"
    );
}
