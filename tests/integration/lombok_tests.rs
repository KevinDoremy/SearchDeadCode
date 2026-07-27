//! Integration tests for Lombok awareness: @Getter/@Setter/@Data
//! generate accessors the graph cannot see, so a field consumed only
//! through them looked dead. A generated-accessor call in the corpus
//! keeps the field; a Lombok field nobody touches is still dead.

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
fn a_field_read_through_a_generated_getter_stays_alive() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Account.java"),
        concat!(
            "package sample;\n\n",
            "@Getter\n",
            "public class Account {\n",
            "    private String nickname;\n",
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
            "        Account account = new Account();\n",
            "        System.out.println(account.getNickname());\n",
            "    }\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("'nickname'"),
        "the generated getter call proves the field lives, stdout was:\n{stdout}"
    );
}

#[test]
fn a_lombok_field_nobody_touches_is_still_dead() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Account.java"),
        concat!(
            "package sample;\n\n",
            "@Getter\n",
            "public class Account {\n",
            "    private String ghost;\n",
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
            "        Account account = new Account();\n",
            "        System.out.println(account);\n",
            "    }\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("'ghost'"),
        "no accessor call anywhere — the annotation alone saves nothing, stdout was:\n{stdout}"
    );
}

#[test]
fn a_builder_write_counts_as_usage() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Order.java"),
        concat!(
            "package sample;\n\n",
            "@Builder\n",
            "public class Order {\n",
            "    private int quantity;\n",
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
            "        Order order = Order.builder().quantity(2).build();\n",
            "        System.out.println(order);\n",
            "    }\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("'quantity'"),
        "the builder setter call proves the field lives, stdout was:\n{stdout}"
    );
}

#[test]
fn a_boolean_is_getter_counts_too() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Session.java"),
        concat!(
            "package sample;\n\n",
            "@Getter\n",
            "public class Session {\n",
            "    private boolean expired;\n",
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
            "        Session session = new Session();\n",
            "        System.out.println(session.isExpired());\n",
            "    }\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("'expired'"),
        "lombok booleans get is-getters, stdout was:\n{stdout}"
    );
}
