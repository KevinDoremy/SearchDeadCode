//! Le run par défaut détecte les situations du repo (mondes jumeaux,
//! classes V1/V2…) et affiche la commande spécialisée prête à copier,
//! paramètres inclus — l'utilisateur n'a pas à connaître les 40 flags
//! ni à deviner les tokens de --compare.

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

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

/// app/main (3 fichiers) + app/mainV2 (3 fichiers) : le layout typique
/// d'une migration à moitié finie.
fn write_twin_dirs_project(dir: &Path) {
    let old = dir.join("app/main");
    let new = dir.join("app/mainV2");
    fs::create_dir_all(&old).unwrap();
    fs::create_dir_all(&new).unwrap();
    fs::write(
        dir.join("Main.kt"),
        "package sample\n\nfun main() {\n    NewHome().show()\n}\n",
    )
    .unwrap();
    for (i, world) in [(0, &old), (1, &new)].iter() {
        let prefix = if *i == 0 { "Old" } else { "New" };
        for name in ["Home", "Menu", "Panel"] {
            fs::write(
                world.join(format!("{prefix}{name}.kt")),
                format!(
                    "package sample.w{i}\n\nclass {prefix}{name} {{\n    fun show() {{}}\n}}\n"
                ),
            )
            .unwrap();
        }
    }
}

#[test]
fn twin_directories_suggest_a_ready_to_copy_compare_command() {
    let temp = tempfile::tempdir().unwrap();
    write_twin_dirs_project(temp.path());

    let stdout = stdout_of(&run(temp.path(), &[]));
    assert!(
        stdout.contains("--compare"),
        "deux arborescences jumelles suggèrent --compare, stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("app/main=") && stdout.contains("app/mainV2"),
        "la commande est prête à copier, tokens inclus, stdout:\n{stdout}"
    );
}

#[test]
fn version_twin_classes_suggest_the_twins_view() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    HomeScreenV2().show()\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("HomeScreen.kt"),
        "package sample\n\nclass HomeScreen {\n    fun show() {}\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("HomeScreenV2.kt"),
        "package sample\n\nclass HomeScreenV2 {\n    fun show() {}\n}\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path(), &[]));
    assert!(
        stdout.contains("--twins"),
        "des classes X/XV2 suggèrent la vue --twins, stdout:\n{stdout}"
    );
}

#[test]
fn a_healthy_project_gets_no_situational_noise() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    Greeter().hello()\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Greeter.kt"),
        "package sample\n\nclass Greeter {\n    fun hello() {}\n}\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path(), &[]));
    assert!(
        !stdout.contains("--compare") && !stdout.contains("--twins"),
        "aucune situation, aucun hint situationnel, stdout:\n{stdout}"
    );
}

#[test]
fn a_lone_android_source_set_does_not_look_like_a_migration() {
    // src/main sans frère src/mainV2 : le source-set standard Android
    // ne doit jamais déclencher le hint migration.
    let temp = tempfile::tempdir().unwrap();
    let main_dir = temp.path().join("src/main");
    fs::create_dir_all(&main_dir).unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    Widget().show()\n}\n",
    )
    .unwrap();
    for name in ["Widget", "Panel", "Menu"] {
        fs::write(
            main_dir.join(format!("{name}.kt")),
            format!("package sample.app\n\nclass {name} {{\n    fun show() {{}}\n}}\n"),
        )
        .unwrap();
    }

    let stdout = stdout_of(&run(temp.path(), &[]));
    assert!(
        !stdout.contains("--compare"),
        "un src/main solitaire n'est pas une migration, stdout:\n{stdout}"
    );
}

#[test]
fn quiet_and_json_outputs_carry_no_hints() {
    let temp = tempfile::tempdir().unwrap();
    write_twin_dirs_project(temp.path());

    let quiet = stdout_of(&run(temp.path(), &["--quiet"]));
    assert!(
        !quiet.contains("--compare"),
        "--quiet reste muet, stdout:\n{quiet}"
    );

    let json = stdout_of(&run(temp.path(), &["--format", "json"]));
    assert!(
        !json.contains("--compare"),
        "la sortie machine ne porte pas de hints, stdout:\n{json}"
    );
}
