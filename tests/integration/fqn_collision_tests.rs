//! Deux surcharges au même niveau de package partagent un FQN. L'index
//! 1:1 ne gardait que la dernière insérée : un appel cross-module (via
//! import) ne liait que le vainqueur de collision et la surcharge
//! publique passait pour morte (îles 3 et 5 de l'audit monorepo — les
//! composables GamesCarouselPost et HorizontalScrollbar).

use std::fs;
use std::path::Path;
use std::process::Output;

fn run(dir: &Path, extra: &[&str]) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .args(extra)
        .output()
        .unwrap()
}

fn overloaded_fixture(dir: &Path) {
    fs::create_dir_all(dir.join("widgets")).unwrap();
    fs::create_dir_all(dir.join("app")).unwrap();
    // La surcharge publique d'abord, la privée ensuite : la privée gagnait
    // la collision d'index (dernier écrit) comme dans l'île 3 réelle.
    fs::write(
        dir.join("widgets/Spinner.kt"),
        concat!(
            "package sample.widgets\n\n",
            "fun Spinner(state: Int) {\n",
            "    println(state)\n",
            "}\n\n",
            "private fun Spinner(alpha: Long) {\n",
            "    println(alpha)\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        dir.join("app/Main.kt"),
        concat!(
            "package sample.app\n\n",
            "import sample.widgets.Spinner\n\n",
            "fun main() {\n",
            "    Spinner(7)\n",
            "}\n",
        ),
    )
    .unwrap();
}

#[test]
fn an_imported_call_reaches_every_overload_sharing_the_fqn() {
    let temp = tempfile::tempdir().unwrap();
    overloaded_fixture(temp.path());

    let stdout = String::from_utf8_lossy(&run(temp.path(), &[]).stdout).to_string();
    assert!(
        !stdout.contains("Spinner"),
        "la surcharge publique appelée via import est vivante, stdout:\n{stdout}"
    );
}

#[test]
fn the_parallel_builder_reaches_every_overload_too() {
    // --incremental false emprunte ParallelGraphBuilder, qui duplique la
    // résolution du builder série — les deux doivent voir tous les porteurs.
    let temp = tempfile::tempdir().unwrap();
    overloaded_fixture(temp.path());

    let stdout =
        String::from_utf8_lossy(&run(temp.path(), &["--incremental", "false"]).stdout).to_string();
    assert!(
        !stdout.contains("Spinner"),
        "le builder parallèle doit lier toutes les surcharges du FQN, stdout:\n{stdout}"
    );
}
