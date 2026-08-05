//! Les méthodes sans corps d'une interface vivante que personne n'appelle.
//!
//! Une mesure comparative annonçait `total_issues: 0` dessus, alors que la
//! lecture de `find_unused_members` les laisse passer tous ses filtres : le
//! parent est atteignable, `Method` n'est pas dans les genres écartés, une
//! déclaration d'interface ne porte pas `override`. Le code disait candidat,
//! la mesure disait zéro.
//!
//! Ce que ces tests ont tranché : les deux disaient vrai, et la variable
//! cachée était le NOM. Le garde `Visibility::Public` (`deep.rs:651`) n'écarte
//! un membre public que s'il est référencé, et la visibilité par défaut du
//! parseur est `Public`. Une méthode nommée `dispose` collecte une arête
//! entrante par homonyme du projet — résolution par nom simple, arêtes
//! marquées `ambiguous` — donc elle passe pour référencée et sort du rapport.
//! Sa voisine au nom unique, dans la même interface, ressort normalement.
//!
//! Le garde reste tel quel : compter une devinette d'homonymie comme une
//! référence garde le symbole VIVANT, et c'est le seul sens d'erreur qu'un
//! détecteur de code mort a le droit de prendre.

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

/// `find_unused_members` n'existe que dans le DeepAnalyzer, et `--deep` vaut
/// `true` par défaut. Sous `--deep=false` les membres d'une classe atteignable
/// ne sont pas examinés du tout : ces fixtures n'auraient rien à dire.
const DEEP: &[&str] = &["--deep=true"];

/// Les messages signalés, tous codes confondus.
fn reported(dir: &Path, extra: &[&str]) -> Vec<String> {
    let mut args = vec!["--format", "json"];
    args.extend_from_slice(extra);
    let out = String::from_utf8_lossy(&run(dir, &args).stdout).to_string();
    let start = out.find('{').unwrap_or(0);
    let parsed: serde_json::Value = serde_json::from_str(&out[start..]).unwrap();
    parsed["issues"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["message"].as_str().unwrap_or("").to_string())
        .collect()
}

/// Une interface vivante (`App` la déclare en type) dont les deux méthodes
/// sans corps ne sont jamais appelées. `dispose` a des homonymes ailleurs,
/// `greetInAVeryUniqueWay` n'en a aucun. Tout le reste est identique entre
/// les deux : même interface, même fichier, même visibilité.
fn write_fixture(dir: &Path) {
    fs::write(
        dir.join("Greeter.kt"),
        "package s\n\
         \n\
         interface Greeter {\n\
         \x20   fun dispose()\n\
         \x20   fun greetInAVeryUniqueWay()\n\
         }\n",
    )
    .unwrap();

    fs::write(
        dir.join("Homonyms.kt"),
        "package s\n\
         \n\
         class Socket {\n\
         \x20   fun dispose() { println(\"socket\") }\n\
         }\n\
         \n\
         class Buffer {\n\
         \x20   fun dispose() { println(\"buffer\") }\n\
         }\n\
         \n\
         class Cursor {\n\
         \x20   fun dispose() { println(\"cursor\") }\n\
         }\n",
    )
    .unwrap();

    fs::write(
        dir.join("Main.kt"),
        "package s\n\
         \n\
         class App(val greeter: Greeter?) {\n\
         \x20   fun close() {\n\
         \x20       Socket().dispose()\n\
         \x20       Buffer().dispose()\n\
         \x20       Cursor().dispose()\n\
         \x20   }\n\
         }\n\
         \n\
         fun main() {\n\
         \x20   App(null).close()\n\
         }\n",
    )
    .unwrap();
}

#[test]
fn a_uniquely_named_bodyless_interface_method_is_reported() {
    let temp = tempfile::tempdir().unwrap();
    write_fixture(temp.path());

    let found = reported(temp.path(), DEEP);
    assert!(
        found.iter().any(|m| m.contains("greetInAVeryUniqueWay")),
        "le code annonçait candidat, il doit tenir parole. Sorti :\n{}",
        found.join("\n")
    );
}

#[test]
fn its_common_named_neighbour_is_not_reported() {
    // Le témoin. Même interface, même visibilité, même absence d'appelant :
    // seul le nom change. Trois `dispose()` sans rapport suffisent à faire
    // passer celui de l'interface pour référencé.
    let temp = tempfile::tempdir().unwrap();
    write_fixture(temp.path());

    let found = reported(temp.path(), DEEP);
    assert!(
        !found
            .iter()
            .any(|m| m.contains("'dispose'") && m.contains("Greeter")),
        "une devinette d'homonymie doit garder vivant, pas tuer. Sorti :\n{}",
        found.join("\n")
    );
}

#[test]
fn removing_the_homonyms_brings_the_common_named_one_back() {
    // La preuve que le nom est bien la seule variable : sans homonyme, la
    // méthode banale ressort exactement comme sa voisine.
    let temp = tempfile::tempdir().unwrap();
    write_fixture(temp.path());
    fs::remove_file(temp.path().join("Homonyms.kt")).unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package s\n\
         \n\
         class App(val greeter: Greeter?)\n\
         \n\
         fun main() {\n\
         \x20   App(null)\n\
         }\n",
    )
    .unwrap();

    let found = reported(temp.path(), DEEP);
    // `method 'dispose'` et pas seulement `dispose` : la trouvaille doit être
    // le MEMBRE. Si l'interface entière tombait, l'assertion passerait pour
    // une raison qui n'a rien à voir avec le nom.
    assert!(
        found.iter().any(|m| m.contains("method 'dispose'")),
        "sans homonyme, plus rien ne la fait passer pour référencée. Sorti :\n{}",
        found.join("\n")
    );
    assert!(
        !found.iter().any(|m| m.contains("'Greeter'")),
        "l'interface doit rester vivante, sinon le test dit autre chose. Sorti :\n{}",
        found.join("\n")
    );
}
