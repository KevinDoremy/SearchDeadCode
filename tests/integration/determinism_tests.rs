//! Deux exécutions identiques doivent rendre la même sortie, octet pour
//! octet.
//!
//! Ce n'est pas de la coquetterie : `scripts/check-corpus.sh` compare la
//! sortie courante à une référence figée, et toute sa valeur tient à ce
//! qu'une sortie vide veuille dire « rien n'a bougé ». Mesuré avant
//! correctif sur les fixtures Kotlin : 26 lignes de diff entre deux runs du
//! rapport standard, jusqu'à 107 pour `--islands`. Un diff de corpus était
//! donc du bruit qu'il fallait lire à la main pour savoir s'il disait quelque
//! chose.
//!
//! Trois sources, toutes de la même famille — un tri partiel appliqué à un
//! ordre d'entrée qui vient d'un `HashSet` :
//!
//! - les trouvailles triées sur (fichier, ligne) seulement, donc deux
//!   paramètres de la même ligne s'échangeaient ;
//! - le récapitulatif « Top Issues » trié sur le compte seul, donc deux
//!   règles à égalité s'échangeaient ;
//! - `dead_clusters` renvoyant ses grappes dans l'ordre du parcours de
//!   l'ensemble, donc les îles et les clusters se renumérotaient.

use std::fs;
use std::path::Path;

fn run(dir: &Path, extra: &[&str]) -> String {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .args(extra)
        .output()
        .unwrap();
    // Les durées varient d'un run à l'autre par nature.
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|l| !l.contains(" in ") || !l.contains('s'))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Assez de matière pour que l'ordre d'un `HashSet` ait de quoi varier :
/// plusieurs fichiers, des homonymes, deux trouvailles sur la même ligne,
/// et des grappes mortes de tailles égales.
fn write_project(dir: &Path) {
    fs::write(
        dir.join("Params.kt"),
        "package s\n\
         \n\
         fun twoOnTheSameLine(alpha: Int, bravo: Int) = 0\n\
         fun alsoTwo(charlie: Int, delta: Int) = 0\n",
    )
    .unwrap();

    // Deux grappes mortes de même taille : de quoi faire hésiter le tri des
    // îles s'il n'est pas total.
    for (n, a, b) in [(1, "AlphaOne", "AlphaTwo"), (2, "BetaOne", "BetaTwo")] {
        fs::write(
            dir.join(format!("Cluster{n}.kt")),
            format!(
                "package s\n\
                 \n\
                 class {a} {{\n\
                 \x20   fun ping() = {b}().pong()\n\
                 }}\n\
                 \n\
                 class {b} {{\n\
                 \x20   fun pong() = {a}().ping()\n\
                 }}\n"
            ),
        )
        .unwrap();
    }

    fs::write(
        dir.join("Homonyms.kt"),
        "package s\n\
         \n\
         class H1 { val scope = 1 }\n\
         class H2 { val scope = 2 }\n\
         class H3 { val scope = 3 }\n",
    )
    .unwrap();

    fs::write(
        dir.join("Main.kt"),
        "package s\n\
         \n\
         fun main() {\n\
         \x20   println(H1().scope + H2().scope + H3().scope)\n\
         \x20   println(twoOnTheSameLine(1, 2) + alsoTwo(3, 4))\n\
         }\n",
    )
    .unwrap();
}

/// Les quatre vues que `check-corpus.sh` fige.
const VIEWS: [&[&str]; 4] = [&[], &["--islands"], &["--clusters"], &["--quick-wins"]];

#[test]
fn every_corpus_view_is_byte_identical_across_runs() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    for view in VIEWS {
        let first = run(temp.path(), view);
        for attempt in 2..=4 {
            assert_eq!(
                first,
                run(temp.path(), view),
                "vue {view:?}, essai {attempt} : la sortie a changé sans que rien ne change"
            );
        }
    }
}

#[test]
fn the_json_report_is_stable_too() {
    // Le JSON alimente la CI et le baseline : un ordre instable y produit un
    // diff à chaque exécution.
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let first = run(temp.path(), &["--format", "json"]);
    assert_eq!(first, run(temp.path(), &["--format", "json"]));
    assert!(first.contains("issues"), "le rapport doit être non vide");
}
