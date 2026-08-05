//! Une devinette d'homonymie ne ressuscite pas le conteneur de son homonyme.
//!
//! Mesuré sur un corpus neutre de 196 fichiers Kotlin : sept des huit objets
//! morts que l'outil manquait portaient un membre nommé `scope`, un nom qui y
//! apparaît 908 fois. La résolution par type échoue, `builder.rs` retombe sur
//! le nom simple et lie tous les homonymes, le membre passe pour atteint, le
//! marquage d'ancêtres remonte jusqu'à l'objet. `--explain` annonçait
//! `Incoming references: 0` et `reachable: yes` sur le même symbole.

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

/// Les trois analyseurs qui marquent des ancêtres, chacun par le drapeau qui
/// l'atteint vraiment. `--deep` vaut `true` par défaut : lancer sans drapeau
/// et avec `--deep` exerce deux fois le même code.
const MODES: [&[&str]; 3] = [
    &["--deep=true"],                // DeepAnalyzer
    &["--deep=false"],               // ReachabilityAnalyzer
    &["--deep=false", "--parallel"], // EnhancedAnalyzer
];

/// Un objet mort dont le seul membre porte `member_name`, et une classe
/// vivante ailleurs qui porte un membre du même nom, lue pour de vrai.
fn write_fixture(dir: &Path, member_name: &str) {
    fs::write(
        dir.join("Dead.kt"),
        format!(
            "package s\n\
             \n\
             object DeadHolder {{\n\
             \x20   val {member_name} = \"nobody reads this\"\n\
             }}\n"
        ),
    )
    .unwrap();

    fs::write(
        dir.join("Live.kt"),
        format!(
            "package s\n\
             \n\
             class LiveThing {{\n\
             \x20   val {member_name} = \"this one is read\"\n\
             }}\n"
        ),
    )
    .unwrap();

    fs::write(
        dir.join("Main.kt"),
        format!(
            "package s\n\
             \n\
             fun main() {{\n\
             \x20   println(LiveThing().{member_name})\n\
             }}\n"
        ),
    )
    .unwrap();
}

#[test]
fn a_dead_object_whose_member_has_a_homonym_is_still_reported() {
    let temp = tempfile::tempdir().unwrap();
    write_fixture(temp.path(), "scope");

    for mode in MODES {
        let found = reported(temp.path(), mode);
        assert!(
            found.iter().any(|m| m.contains("DeadHolder")),
            "mode {mode:?} : l'homonyme de `scope` ne doit pas garder l'objet en vie. Sorti :\n{}",
            found.join("\n")
        );
    }
}

#[test]
fn the_same_fixture_with_a_unique_member_name_reports_the_same_thing() {
    // Le témoin. Si les deux fixtures ne disaient pas la même chose, le
    // correctif tiendrait à autre chose qu'à l'homonymie.
    let temp = tempfile::tempdir().unwrap();
    write_fixture(temp.path(), "aVeryUniqueMemberName");

    for mode in MODES {
        let found = reported(temp.path(), mode);
        assert!(
            found.iter().any(|m| m.contains("DeadHolder")),
            "mode {mode:?} : sorti :\n{}",
            found.join("\n")
        );
    }
}

#[test]
fn the_contamination_does_not_survive_one_more_hop() {
    // Le cas qu'un garde LOCAL rate. Écarter le seul `DeadHolder.scope` le
    // laisse dans l'ensemble atteignable, donc ses arêtes sortantes sont
    // suivies, donc `buriedDeeper` reçoit une arête entrante non ambiguë et
    // récupère le droit de marquer ses ancêtres — ce qui ressuscite
    // `SecondHolder`. Seule la fermeture transitive tue les deux.
    //
    // L'import du membre est ce qui rend le cas visible : écrire
    // `SecondHolder.buriedDeeper()` poserait une arête directe vers le
    // qualificateur, et l'objet serait atteint sans passer par les ancêtres.
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Second.kt"),
        "package s\n\
         \n\
         object SecondHolder {\n\
         \x20   fun buriedDeeper() = 42\n\
         }\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Dead.kt"),
        "package s\n\
         \n\
         import s.SecondHolder.buriedDeeper\n\
         \n\
         object DeadHolder {\n\
         \x20   val scope = buriedDeeper()\n\
         }\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Live.kt"),
        "package s\n\
         \n\
         class LiveThing {\n\
         \x20   val scope = \"this one is read\"\n\
         }\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package s\n\
         \n\
         fun main() {\n\
         \x20   println(LiveThing().scope)\n\
         }\n",
    )
    .unwrap();

    for mode in MODES {
        let found = reported(temp.path(), mode);
        for name in ["DeadHolder", "SecondHolder"] {
            assert!(
                found.iter().any(|m| m.contains(name)),
                "mode {mode:?} : {name} devait tomber aussi. Sorti :\n{}",
                found.join("\n")
            );
        }
    }
}

#[test]
fn an_unambiguous_edge_still_keeps_its_container_alive() {
    // Le sens d'erreur. Un seul porteur de ce nom dans tout le projet : la
    // résolution n'a rien deviné, l'arête n'est pas ambiguë, le conteneur
    // reste vivant.
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Holder.kt"),
        "package s\n\
         \n\
         object ConfigHolder {\n\
         \x20   val theOnlyOneWithThisName = 7\n\
         }\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package s\n\
         \n\
         fun main() {\n\
         \x20   println(ConfigHolder.theOnlyOneWithThisName)\n\
         }\n",
    )
    .unwrap();

    for mode in MODES {
        let found = reported(temp.path(), mode);
        assert!(
            !found.iter().any(|m| m.contains("ConfigHolder")),
            "mode {mode:?} : une référence résolue garde vivant. Sorti :\n{}",
            found.join("\n")
        );
    }
}

#[test]
fn a_member_imported_by_name_keeps_its_container_alive() {
    // Le faux positif que le correctif d'ancêtres a failli livrer, et le
    // seul de la campagne qui aurait cassé une compilation.
    //
    // `import s.ConfigHolder.scope` puis `println(scope)` : le conteneur
    // n'est nommé NULLE PART ailleurs, donc sa seule chance de rester vivant
    // passe par le marquage d'ancêtres depuis `scope`. Or un membre est
    // indexé sous son propre FQN, pas sous le chemin pointé de l'import : la
    // recherche exacte ratait, la référence retombait sur le nom simple et
    // liait les trois `scope` du projet par une arête ambiguë. Le garde
    // d'ancêtres condamnait alors l'objet, et `--delete` aurait supprimé du
    // code utilisé. Le correctif fait descendre le chemin pointé, comme la
    // branche alias le faisait déjà.
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Holder.kt"),
        "package s\n\
         \n\
         object ConfigHolder {\n\
         \x20   val scope = \"used for real\"\n\
         }\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Noise.kt"),
        "package s\n\
         \n\
         class N1 { val scope = 1 }\n\
         class N2 { val scope = 2 }\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package s\n\
         \n\
         import s.ConfigHolder.scope\n\
         \n\
         fun main() {\n\
         \x20   println(scope)\n\
         \x20   println(N1().scope + N2().scope)\n\
         }\n",
    )
    .unwrap();

    for mode in MODES {
        let found = reported(temp.path(), mode);
        assert!(
            !found.iter().any(|m| m.contains("ConfigHolder")),
            "mode {mode:?} : l'objet est utilisé, le supprimer casserait le build. Sorti :\n{}",
            found.join("\n")
        );
    }
}
