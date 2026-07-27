//! Integration tests for --dead-accessors: JavaBean properties where
//! the getter is never called. Written-but-never-read means the whole
//! write pipeline runs for nothing; neither-called means field plus
//! both accessors can go together — something per-symbol reports
//! cannot say because the field is "used" by its own accessors.

use std::fs;
use std::path::Path;
use std::process::Output;

fn run(dir: &Path) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .arg("--dead-accessors")
        .output()
        .unwrap()
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn user_bean(dir: &Path) {
    fs::write(
        dir.join("User.java"),
        concat!(
            "package sample;\n\n",
            "public class User {\n",
            "    private String nickname;\n\n",
            "    public String getNickname() {\n",
            "        return nickname;\n",
            "    }\n\n",
            "    public void setNickname(String value) {\n",
            "        this.nickname = value;\n",
            "    }\n",
            "}\n",
        ),
    )
    .unwrap();
}

#[test]
fn a_write_only_bean_property_is_flagged() {
    let temp = tempfile::tempdir().unwrap();
    user_bean(temp.path());
    fs::write(
        temp.path().join("Main.java"),
        concat!(
            "package sample;\n\n",
            "public class Main {\n",
            "    public static void main(String[] args) {\n",
            "        User user = new User();\n",
            "        user.setNickname(\"ghost\");\n",
            "    }\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("nickname") && stdout.to_lowercase().contains("write-only"),
        "written but never read is called out, stdout was:\n{stdout}"
    );
}

#[test]
fn a_read_and_written_property_is_not_flagged() {
    let temp = tempfile::tempdir().unwrap();
    user_bean(temp.path());
    fs::write(
        temp.path().join("Main.java"),
        concat!(
            "package sample;\n\n",
            "public class Main {\n",
            "    public static void main(String[] args) {\n",
            "        User user = new User();\n",
            "        user.setNickname(\"ghost\");\n",
            "        System.out.println(user.getNickname());\n",
            "    }\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("nickname"),
        "a read property is healthy, stdout was:\n{stdout}"
    );
}

#[test]
fn a_never_touched_property_is_grouped_as_dead() {
    let temp = tempfile::tempdir().unwrap();
    user_bean(temp.path());
    fs::write(
        temp.path().join("Main.java"),
        concat!(
            "package sample;\n\n",
            "public class Main {\n",
            "    public static void main(String[] args) {\n",
            "        User user = new User();\n",
            "        System.out.println(user);\n",
            "    }\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("nickname") && stdout.to_lowercase().contains("dead"),
        "field and both accessors die together, stdout was:\n{stdout}"
    );
}

#[test]
fn direct_field_reads_keep_the_property_alive() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Point.java"),
        concat!(
            "package sample;\n\n",
            "public class Point {\n",
            "    public int weight;\n\n",
            "    public int getWeight() {\n",
            "        return weight;\n",
            "    }\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.java"),
        concat!(
            "package sample;\n\n",
            "public class Main {\n",
            "    public static void main(String[] args) {\n",
            "        Point point = new Point();\n",
            "        System.out.println(point.weight);\n",
            "    }\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("weight"),
        "a direct field read is a read, stdout was:\n{stdout}"
    );
}

#[test]
fn no_bean_properties_is_a_clean_answer() {
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
        "no beans is fine, output was:\n{output:?}"
    );
    assert!(
        stdout.to_lowercase().contains("no dead accessor"),
        "the verdict is explicit, stdout was:\n{stdout}"
    );
}
