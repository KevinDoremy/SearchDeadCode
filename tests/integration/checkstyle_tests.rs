//! Checkstyle XML : le format que l'écosystème d'analyse statique parle déjà.
//!
//! Choisi contre JUnit XML, et pour une raison vérifiable : detekt — l'outil
//! auquel les équipes comparent celui-ci — publie exactement ce format pour la
//! CI, et Jenkins le lit nativement via Warnings Next Generation, comme
//! SonarQube. JUnit aurait atteint les mêmes plateformes en déposant les
//! trouvailles parmi les tests en échec, ce qui pollue les métriques et
//! l'historique de tests avec ce qui n'est pas un test.

use std::fs;
use std::path::Path;
use std::process::Command;

fn run(dir: &Path, extra: &[&str]) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .args(extra)
        // Pas de cache : il déposerait un fichier dans le tempdir et l'un des
        // tests compte précisément ce qui s'y trouve.
        .arg("--incremental=false")
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).to_string()
}

fn write_dead(dir: &Path) {
    fs::write(
        dir.join("App.kt"),
        "package s\n\
         \n\
         class OldHelper {\n\
         \x20   fun helper() = 1\n\
         }\n\
         \n\
         class AlsoDead\n\
         \n\
         fun main() {\n\
         \x20   println(\"boot\")\n\
         }\n",
    )
    .unwrap();
}

#[test]
fn one_error_element_per_finding_carrying_its_rule_code() {
    let temp = tempfile::tempdir().unwrap();
    write_dead(temp.path());

    let xml = run(temp.path(), &["--format", "checkstyle"]);

    assert_eq!(
        xml.matches("<error ").count(),
        2,
        "un <error> par trouvaille. Sorti :\n{xml}"
    );
    assert!(
        xml.contains("source=\"DC001\""),
        "le code de règle est ce que Warnings NG affiche en catégorie. Sorti :\n{xml}"
    );
    assert!(
        xml.contains("severity=\"warning\""),
        "les consommateurs filtrent sur ces orthographes exactes. Sorti :\n{xml}"
    );
}

#[test]
fn a_clean_project_still_produces_a_parsable_document() {
    // Le cas qui casse les consommateurs : un fichier de zéro octet fait
    // échouer le parseur le jour où le projet est sain.
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("App.kt"),
        "package s\n\nfun main() {\n    println(1)\n}\n",
    )
    .unwrap();

    let xml = run(temp.path(), &["--format", "checkstyle"]);

    assert!(xml.starts_with("<?xml version=\"1.0\""), "Sorti :\n{xml}");
    assert!(xml.contains("</checkstyle>"), "Sorti :\n{xml}");
    assert!(!xml.contains("<error "), "Sorti :\n{xml}");
}

#[test]
fn a_quoted_symbol_name_cannot_break_the_attribute() {
    // Les messages citent le symbole (`class 'Foo' is never used`) : sans
    // échappement de l'apostrophe, l'attribut se referme au mauvais endroit.
    let temp = tempfile::tempdir().unwrap();
    write_dead(temp.path());

    let xml = run(temp.path(), &["--format", "checkstyle"]);

    assert!(
        xml.contains("&apos;OldHelper&apos;"),
        "l'apostrophe doit être échappée. Sorti :\n{xml}"
    );
    assert!(
        !xml.contains("message=\"class 'OldHelper'"),
        "et jamais laissée brute. Sorti :\n{xml}"
    );
}

#[test]
fn the_document_is_stable_across_runs() {
    // Même exigence que le rapport terminal : un ordre instable transforme un
    // diff de CI en bruit.
    let temp = tempfile::tempdir().unwrap();
    write_dead(temp.path());

    let first = run(temp.path(), &["--format", "checkstyle"]);
    for _ in 0..3 {
        assert_eq!(first, run(temp.path(), &["--format", "checkstyle"]));
    }
    assert!(
        first.contains("<error "),
        "le test doit porter sur du contenu"
    );
}
