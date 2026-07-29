//! Integration tests for --why-alive <symbol>: the inverse of --explain.
//! "Why is my legacy NOT detected?" is the first question on a real
//! project — the answer is the retention chain from a root.

use std::fs;
use std::path::Path;
use std::process::Output;

fn write_project(dir: &Path) {
    fs::write(
        dir.join("Chain.kt"),
        concat!(
            "package sample\n\n",
            "class LegacyEngine {\n",
            "    fun turnOver() {}\n",
            "}\n\n",
            "class Garage {\n",
            "    fun service() {\n",
            "        LegacyEngine().turnOver()\n",
            "    }\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        dir.join("Main.kt"),
        concat!(
            "package sample\n\n",
            "fun main() {\n",
            "    Garage().service()\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        dir.join("Corpse.kt"),
        "package sample\n\nclass Corpse {\n    fun rot() {}\n}\n",
    )
    .unwrap();
}

fn run(dir: &Path, symbol: &str) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .arg("--why-alive")
        .arg(symbol)
        .output()
        .unwrap()
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn the_chain_from_the_root_is_shown() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let stdout = stdout_of(&run(temp.path(), "LegacyEngine"));
    assert!(
        stdout.contains("Garage"),
        "the keeper appears in the chain, stdout was:\n{stdout}"
    );
    assert!(
        stdout.contains("LegacyEngine"),
        "the symbol itself appears, stdout was:\n{stdout}"
    );
}

#[test]
fn a_dead_symbol_is_redirected_to_explain() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let stdout = stdout_of(&run(temp.path(), "Corpse"));
    assert!(
        stdout.to_lowercase().contains("dead") && stdout.contains("--explain"),
        "a corpse has no liveness to explain, point to --explain, stdout was:\n{stdout}"
    );
}

#[test]
fn an_unknown_symbol_says_so() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let stdout = stdout_of(&run(temp.path(), "NoSuchThing"));
    assert!(
        stdout.contains("not found"),
        "unknown symbols are named as such, stdout was:\n{stdout}"
    );
}

#[test]
fn member_retention_shows_the_member_of_link() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Garage.kt"),
        concat!(
            "package sample\n\n",
            "class Garage {\n",
            "    fun service() {}\n\n",
            "    fun neverCalled() {}\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    Garage().service()\n}\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path(), "neverCalled"));
    assert!(
        stdout.contains("member of") || stdout.to_lowercase().contains("dead"),
        "either the member-of chain or an honest death verdict, stdout was:\n{stdout}"
    );
}

#[test]
fn a_root_explains_itself() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let stdout = stdout_of(&run(temp.path(), "main"));
    assert!(
        stdout.to_lowercase().contains("entry point") || stdout.to_lowercase().contains("root"),
        "main is its own reason, stdout was:\n{stdout}"
    );
}

#[test]
fn a_root_names_the_annotation_that_makes_it_one() {
    // « It is itself an entry point » sans la cause est inutilisable sur
    // un vrai projet : la réponse utile est QUELLE règle fait la racine
    // (ici @Inject sur le constructeur — rétention DI).
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Controller.kt"),
        concat!(
            "package sample\n\n",
            "class PushController @Inject constructor() {\n",
            "    fun relay() {}\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"up\")\n}\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path(), "PushController"));
    assert!(
        stdout.contains("@Inject"),
        "la racine nomme l'annotation qui la retient, stdout:\n{stdout}"
    );
}
