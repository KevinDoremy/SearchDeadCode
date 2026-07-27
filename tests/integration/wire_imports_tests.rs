//! Integration tests for DC012 (duplicate import) and DC013 (redundant
//! null init), both implemented long ago but never called from main.
//!
//! DC013's original premise was inverted: Kotlin properties REQUIRE an
//! initializer, so `var x: String? = null` is not redundant there. Java
//! fields default to null (JLS 4.12.5), so `private String x = null;`
//! is — unless the field is final, where dropping the initializer
//! breaks definite assignment.

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
fn a_duplicated_import_is_reported() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        concat!(
            "package sample\n\n",
            "import kotlin.math.abs\n",
            "import kotlin.math.abs\n\n",
            "fun main() {\n",
            "    println(abs(-1))\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("DC012"),
        "the second import of kotlin.math.abs is a duplicate, stdout was:\n{stdout}"
    );
}

#[test]
fn unique_imports_stay_silent() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        concat!(
            "package sample\n\n",
            "import kotlin.math.abs\n",
            "import kotlin.math.max\n\n",
            "fun main() {\n",
            "    println(max(abs(-1), 2))\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("DC012"),
        "two different imports are fine, stdout was:\n{stdout}"
    );
}

#[test]
fn a_java_field_initialized_to_null_is_reported() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Holder.java"),
        concat!(
            "package sample;\n\n",
            "public class Holder {\n",
            "    private String cache = null;\n\n",
            "    public String cached() {\n",
            "        return cache;\n",
            "    }\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(Holder().cached())\n}\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("DC013"),
        "Java fields default to null, stdout was:\n{stdout}"
    );
}

#[test]
fn a_java_local_initialized_to_null_is_not_a_field() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Worker.java"),
        concat!(
            "package sample;\n\n",
            "public class Worker {\n",
            "    public String work() {\n",
            "        String result = null;\n",
            "        result = \"done\";\n",
            "        return result;\n",
            "    }\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(Worker().work())\n}\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("DC013"),
        "locals need definite assignment, null init is meaningful, stdout was:\n{stdout}"
    );
}

#[test]
fn a_final_java_field_keeps_its_null() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Frozen.java"),
        concat!(
            "package sample;\n\n",
            "public class Frozen {\n",
            "    private final String nothing = null;\n\n",
            "    public String peek() {\n",
            "        return nothing;\n",
            "    }\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(Frozen().peek())\n}\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("DC013"),
        "a final field cannot lose its initializer, stdout was:\n{stdout}"
    );
}

#[test]
fn a_kotlin_nullable_property_is_required_to_initialize() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("State.kt"),
        concat!(
            "package sample\n\n",
            "class State {\n",
            "    var name: String? = null\n\n",
            "    fun label() = name ?: \"anonymous\"\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(State().label())\n}\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("DC013"),
        "Kotlin properties must be initialized, nothing is redundant, stdout was:\n{stdout}"
    );
}
