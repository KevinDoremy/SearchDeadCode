//! Integration tests for --fail-on-findings: granular exit codes make
//! the tool scriptable without parsing output. 0 = clean, 1 = findings
//! (this flag), 2 = config error, 3 = ratchet/necromancy gate.

use std::fs;
use std::path::Path;
use std::process::Output;

fn bin(dir: &Path, args: &[&str]) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn findings_exit_one_with_the_flag() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Ghost.kt"),
        "package sample\n\nclass Ghost {\n    fun haunt() {}\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();

    let out = bin(temp.path(), &["--fail-on-findings"]);
    assert_eq!(
        out.status.code(),
        Some(1),
        "findings + flag = exit 1, output was:\n{out:?}"
    );
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(
        stdout.contains("'Ghost'"),
        "the report still prints before failing, stdout was:\n{stdout}"
    );
}

#[test]
fn a_clean_project_exits_zero_with_the_flag() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();

    let out = bin(temp.path(), &["--fail-on-findings"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "clean + flag = exit 0, output was:\n{out:?}"
    );
}

#[test]
fn without_the_flag_findings_still_exit_zero() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Ghost.kt"),
        "package sample\n\nclass Ghost {\n    fun haunt() {}\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();

    let out = bin(temp.path(), &[]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "the default stays non-breaking, output was:\n{out:?}"
    );
}

#[test]
fn a_deletion_that_ran_disarms_the_gate_a_dry_run_does_not() {
    // Un pipeline d'auto-delete : la suppression résout les trouvailles,
    // sortir 1 ensuite bloquerait l'étape commit qui suit. Le dry-run ne
    // touche à rien, donc il gate normalement.
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Ghost.kt"),
        "package sample\n\nclass Ghost {\n    fun haunt() {}\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();

    let dry = bin(temp.path(), &["--profile", "ci", "--delete", "--dry-run"]);
    assert_eq!(
        dry.status.code(),
        Some(1),
        "dry-run : rien n'est résolu, la porte ferme. Output:\n{dry:?}"
    );

    let deleted = bin(temp.path(), &["--profile", "ci", "--delete", "--yes"]);
    assert_eq!(
        deleted.status.code(),
        Some(0),
        "suppression faite : les trouvailles sont résolues, exit 0. Output:\n{deleted:?}"
    );
    let remaining = fs::read_to_string(temp.path().join("Ghost.kt")).unwrap_or_default();
    assert!(
        !remaining.contains("class Ghost"),
        "et la classe est bien partie du fichier"
    );
}

#[test]
fn a_corrupt_baseline_is_exit_2_not_a_burst_of_findings() {
    // Le contrat documenté : 2 = l'outil n'a pas pu travailler. Avant, un
    // baseline illisible hors --ratchet donnait un warning + le rapport NON
    // filtré → exit 1 sous la porte — la CI criait « du code mort » quand le
    // vrai problème était le fichier.
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Ghost.kt"),
        "package sample\n\nclass Ghost {\n    fun haunt() {}\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();
    let baseline = temp.path().join("baseline.json");
    fs::write(&baseline, "{ this is not json").unwrap();

    let out = bin(
        temp.path(),
        &[
            "--baseline",
            baseline.to_str().unwrap(),
            "--fail-on-findings",
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(2),
        "présent mais inutilisable = erreur d'outillage. Output:\n{out:?}"
    );
}

#[test]
fn baselined_findings_do_not_fail_the_gate() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Ghost.kt"),
        "package sample\n\nclass Ghost {\n    fun haunt() {}\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();
    let baseline = temp.path().join("baseline.json");
    let seed = bin(
        temp.path(),
        &["--generate-baseline", baseline.to_str().unwrap()],
    );
    assert!(seed.status.success());

    let out = bin(
        temp.path(),
        &[
            "--baseline",
            baseline.to_str().unwrap(),
            "--fail-on-findings",
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "the gate judges NEW findings only, output was:\n{out:?}"
    );
}
