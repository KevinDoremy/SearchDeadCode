//! Integration tests for dead @Composable detection: a composable
//! nobody calls is dead code like any other function — blanket
//! retention of the Composable annotation hid a whole class of
//! findings. @Preview stays a root (the IDE calls it), and anything a
//! preview calls stays transitively alive.

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
fn a_composable_nobody_calls_is_flagged() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Widgets.kt"),
        concat!(
            "package sample\n\n",
            "@Composable\n",
            "fun LoneBanner() {\n",
            "    Text(\"nobody renders me\")\n",
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
        stdout.contains("'LoneBanner'"),
        "an uncalled composable is dead code, stdout was:\n{stdout}"
    );
}

#[test]
fn a_called_composable_is_alive() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Widgets.kt"),
        concat!(
            "package sample\n\n",
            "@Composable\n",
            "fun StatusBanner() {\n",
            "    Text(\"status\")\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        concat!(
            "package sample\n\n",
            "fun main() {\n",
            "    StatusBanner()\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("'StatusBanner'"),
        "a rendered composable is alive, stdout was:\n{stdout}"
    );
}

#[test]
fn a_preview_function_is_retained() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Previews.kt"),
        concat!(
            "package sample\n\n",
            "@Preview\n",
            "@Composable\n",
            "fun GreetingPreview() {\n",
            "    Text(\"preview\")\n",
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
        !stdout.contains("'GreetingPreview'"),
        "the IDE calls previews — they are roots, stdout was:\n{stdout}"
    );
}

#[test]
fn a_composable_called_only_from_a_preview_stays_alive() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Card.kt"),
        concat!(
            "package sample\n\n",
            "@Composable\n",
            "fun ProfileCard() {\n",
            "    Text(\"profile\")\n",
            "}\n\n",
            "@Preview\n",
            "@Composable\n",
            "fun ProfileCardPreview() {\n",
            "    ProfileCard()\n",
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
        !stdout.contains("'ProfileCard'"),
        "reachable from a preview root — conservatively alive, stdout was:\n{stdout}"
    );
}

#[test]
fn a_private_composable_nobody_calls_is_flagged_too() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Internal.kt"),
        concat!(
            "package sample\n\n",
            "@Composable\n",
            "private fun HiddenChip() {\n",
            "    Text(\"never shown\")\n",
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
        stdout.contains("'HiddenChip'"),
        "visibility does not resurrect a dead composable, stdout was:\n{stdout}"
    );
}
