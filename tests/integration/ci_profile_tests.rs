//! `--profile ci` : le réglage de pipeline en une commande.
//!
//! Le profil existait et ne faisait qu'une chose — passer la confiance à
//! `high`. Il ne posait pas la porte, ne choisissait rien, et laissait un
//! fichier de cache dans l'espace de travail à chaque run (221 Mo mesurés sur
//! un projet de 9135 fichiers, pour économiser 167 s sur 330).
//!
//! Pire, le seuil `high` le rendait aveugle à sa propre raison d'être : sur ce
//! même projet il ne voyait que 126 trouvailles sur 2058, dont 79 de DC013,
//! une règle cosmétique. Une classe morte fraîchement poussée est signalée en
//! `medium`. Le bruit est le travail du baseline, pas celui du seuil.

use std::fs;
use std::path::Path;
use std::process::Command;

/// Le code de sortie, qui EST le contrat de CI. Surtout pas une sortie
/// tuyautée : un `$?` derrière un `grep` rend le code du grep, et c'est
/// exactement l'erreur qui a failli faire conclure que la porte ne fermait
/// pas.
fn exit_code(dir: &Path, args: &[&str]) -> i32 {
    Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .args(args)
        .output()
        .unwrap()
        .status
        .code()
        .unwrap_or(-1)
}

fn stdout(dir: &Path, args: &[&str]) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .args(args)
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).to_string()
}

/// Une dette existante d'un symbole, plus un point d'entrée.
fn write_project(dir: &Path) {
    fs::write(
        dir.join("App.kt"),
        "package s\n\
         \n\
         class OldHelper {\n\
         \x20   fun helper() = 1\n\
         }\n\
         \n\
         fun main() {\n\
         \x20   println(\"boot\")\n\
         }\n",
    )
    .unwrap();
}

fn add_new_dead_code(dir: &Path) {
    let mut src = fs::read_to_string(dir.join("App.kt")).unwrap();
    src.push_str("\nclass BrandNewDead {\n    fun x() = 2\n}\n");
    fs::write(dir.join("App.kt"), src).unwrap();
}

fn freeze_debt(dir: &Path) {
    // Le nom conventionnel, celui que le profil ramasse tout seul. Chemin
    // ABSOLU : `--generate-baseline` écrit relativement au répertoire courant
    // du processus, pas au dossier analysé, et un chemin relatif ici déposait
    // le fichier dans le dépôt au lieu du tempdir.
    let baseline = dir.join(".deadcode-baseline.json");
    let code = exit_code(dir, &["--generate-baseline", baseline.to_str().unwrap()]);
    assert_eq!(code, 0, "générer un baseline ne doit pas casser");
    assert!(
        baseline.is_file(),
        "le baseline doit être écrit à côté du projet"
    );
}

#[test]
fn the_profile_gates_the_build() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    assert_eq!(
        exit_code(temp.path(), &["--profile", "ci"]),
        1,
        "un drapeau nommé pour les pipelines doit fermer la porte"
    );
}

#[test]
fn the_profile_leaves_no_cache_in_the_workspace() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    exit_code(temp.path(), &["--profile", "ci"]);

    assert!(
        !temp.path().join(".searchdeadcode-cache.json").exists(),
        "un checkout de CI est neuf : le cache ne lui apprend rien et \
         encombre l'espace de travail"
    );
}

#[test]
fn the_profile_picks_up_a_committed_baseline_without_being_told() {
    // Le cœur de l'affaire : la ligne de CI doit rester `--profile ci`, sans
    // avoir à nommer un fichier dont le projet a déjà décidé.
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());
    freeze_debt(temp.path());

    assert_eq!(
        exit_code(temp.path(), &["--profile", "ci"]),
        0,
        "la dette gelée ne doit plus casser le build"
    );

    add_new_dead_code(temp.path());

    assert_eq!(
        exit_code(temp.path(), &["--profile", "ci"]),
        1,
        "mais ce que la branche ajoute, oui"
    );
    let report = stdout(temp.path(), &["--profile", "ci"]);
    assert!(
        report.contains("BrandNewDead"),
        "et le rapport nomme le coupable. Sorti :\n{report}"
    );
    assert!(
        !report.contains("OldHelper"),
        "sans rejouer la dette déjà acceptée. Sorti :\n{report}"
    );
}

#[test]
fn a_baseline_is_never_invented_outside_the_profile() {
    // Hors profil, un `.deadcode-baseline.json` qui traîne ne doit pas
    // silencieusement filtrer un run : qui n'a pas demandé de baseline veut
    // voir son projet en entier.
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());
    freeze_debt(temp.path());

    let report = stdout(temp.path(), &[]);
    assert!(
        report.contains("OldHelper"),
        "sans --profile ci, le baseline posé à côté ne s'applique pas. Sorti :\n{report}"
    );
}

#[test]
fn an_explicit_flag_beats_the_profile() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    assert_eq!(
        exit_code(
            temp.path(),
            &["--profile", "ci", "--fail-on-findings=false"]
        ),
        0,
        "regarder sans casser le build doit rester possible"
    );

    exit_code(
        temp.path(),
        &[
            "--profile",
            "ci",
            "--incremental=true",
            "--fail-on-findings=false",
        ],
    );
    assert!(
        temp.path().join(".searchdeadcode-cache.json").exists(),
        "et redemander le cache aussi"
    );
}

#[test]
fn the_profile_still_sees_a_plain_dead_class() {
    // La régression que le seuil `high` produisait : DC001 sort en `medium`,
    // donc le préréglage « strict » ne voyait pas une classe morte poussée
    // dans la branche — précisément ce qu'une porte de pipeline doit voir.
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let report = stdout(
        temp.path(),
        &["--profile", "ci", "--fail-on-findings=false"],
    );
    assert!(
        report.contains("OldHelper"),
        "une classe morte ordinaire doit rester visible sous le profil. Sorti :\n{report}"
    );
}

#[test]
fn the_bare_gate_flag_does_not_swallow_the_path() {
    // La régression que le passage à Option<bool> a failli livrer : sans
    // `require_equals`, clap consommait le token suivant comme valeur du
    // drapeau — et `--fail-on-findings <path>` est EXACTEMENT l'ordre dans
    // lequel l'action GitHub construit sa commande. `require_equals` laisse
    // la forme nue valide et le chemin positionnel.
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let code = Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .args(["--min-confidence", "medium", "--fail-on-findings"])
        .arg(temp.path())
        .output()
        .unwrap()
        .status
        .code()
        .unwrap_or(-1);
    assert_eq!(code, 1, "flag nu puis chemin : la porte ferme sur la dette");
}

#[test]
fn a_nonexistent_path_is_exit_2_never_a_clean_report() {
    // La promesse centrale du contrat de codes de sortie. Avant : un chemin
    // inexistant rendait « No Kotlin or Java files found » et sortait 0 — une
    // CI concluait « aucun code mort » sur un checkout raté ou une faute de
    // frappe. 2 = l'outil n'a pas pu travailler, jamais un rapport propre.
    let code = exit_code(Path::new("/nonexistent/searchdeadcode/test/path"), &[]);
    assert_eq!(code, 2, "un chemin inexistant est une erreur d'outillage");

    // Un dossier existant sans sources, lui, reste un projet vide légitime.
    let empty = tempfile::tempdir().unwrap();
    assert_eq!(
        exit_code(empty.path(), &["--profile", "ci"]),
        0,
        "un dossier vide n'est pas une erreur"
    );
}

#[test]
fn generating_a_baseline_disarms_the_gate_for_that_run() {
    // Geler la dette est un acte d'acceptation : sortir 1 dans la seconde qui
    // suit ferait échouer l'étape d'adoption que la doc CI ouvre.
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());
    let baseline = temp.path().join(".deadcode-baseline.json");

    let code = exit_code(
        temp.path(),
        &[
            "--profile",
            "ci",
            "--generate-baseline",
            baseline.to_str().unwrap(),
        ],
    );
    assert_eq!(code, 0, "le run qui gèle la dette ne casse pas");
    assert!(baseline.is_file(), "et le baseline est bien écrit");
}
