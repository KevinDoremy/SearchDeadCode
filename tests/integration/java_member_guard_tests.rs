//! Integration tests for the Java private-member guards on DC001.
//!
//! A private field of a `Serializable` class is read reflectively, an
//! annotated field is reached by its wire name, and a used overload keeps
//! its same-name siblings alive. None of these guards may silence a
//! genuinely dead private member of an ordinary class.

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
fn a_private_field_of_a_serializable_class_is_reflectively_reachable() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("SessionState.java"),
        concat!(
            "package sample;\n\n",
            "import java.io.Serializable;\n\n",
            "public class SessionState implements Serializable {\n",
            "    private long legacyStamp;\n\n",
            "    public String describe() {\n",
            "        return \"state\";\n",
            "    }\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Caller.java"),
        concat!(
            "package sample;\n\n",
            "public class Caller {\n",
            "    public static void main(String[] args) {\n",
            "        System.out.println(new SessionState().describe());\n",
            "    }\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("legacyStamp"),
        "Java serialization reads private fields without naming them, stdout was:\n{stdout}"
    );
}

#[test]
fn a_serialized_name_field_is_a_dto_verdict_not_dc001() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("UserDto.java"),
        concat!(
            "package sample;\n\n",
            "public class UserDto {\n",
            "    @SerializedName(\"display_name\")\n",
            "    private String displayName;\n\n",
            "    public String describe() {\n",
            "        return \"user\";\n",
            "    }\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(UserDto().describe())\n}\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("[DC001] field 'displayName'"),
        "a Gson-filled field is not an unreferenced declaration, stdout was:\n{stdout}"
    );
}

#[test]
fn a_used_overload_keeps_its_siblings_alive() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Formatter.java"),
        concat!(
            "package sample;\n\n",
            "public class Formatter {\n",
            "    public static String render(String value) {\n",
            "        return value;\n",
            "    }\n\n",
            "    public static String render(int value) {\n",
            "        return String.valueOf(value);\n",
            "    }\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Caller.java"),
        concat!(
            "package sample;\n\n",
            "public class Caller {\n",
            "    public static void main(String[] args) {\n",
            "        System.out.println(Formatter.render(\"x\"));\n",
            "    }\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("render"),
        "name-level resolution cannot pick an overload, both stay alive, stdout was:\n{stdout}"
    );
}

#[test]
fn a_dead_private_member_of_an_ordinary_class_is_still_reported() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Plain.java"),
        concat!(
            "package sample;\n\n",
            "public class Plain {\n",
            "    private long forgotten;\n\n",
            "    public String describe() {\n",
            "        return \"plain\";\n",
            "    }\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Caller.java"),
        concat!(
            "package sample;\n\n",
            "public class Caller {\n",
            "    public static void main(String[] args) {\n",
            "        System.out.println(new Plain().describe());\n",
            "    }\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("forgotten"),
        "the guards must not silence a dead field of an ordinary class, stdout was:\n{stdout}"
    );
}
