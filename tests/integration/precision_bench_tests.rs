//! Ground-truth precision/recall bench: a fixture whose dead and
//! living symbols are known by construction, measured through the real
//! binary. The floors are a quality ratchet — a detector change that
//! silently trades precision for recall (or the reverse) fails here.

use std::fs;
use std::path::Path;
use std::process::Output;

fn write_file(root: &Path, rel: &str, content: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

/// Symbols dead by construction.
const DEAD: &[&str] = &[
    "OrphanHelper",
    "GhostMapper",
    "unusedTopLevel",
    "ZombieLeaf",
];
/// Symbols alive by construction (reachable from main or entry points).
/// Reflected (string-referenced) is deliberately absent from both
/// lists: the default profile reports it with elevated risk — that is
/// the documented contract, neither a hit nor a miss here.
const ALIVE: &[&str] = &["Engine", "start", "MainActivity"];

fn ground_truth_project(root: &Path) {
    // living core: main -> Engine.start
    write_file(
        root,
        "src/main/kotlin/Main.kt",
        concat!(
            "package sample\n\n",
            "fun main() {\n",
            "    Engine().start()\n",
            "    val name = \"Reflected\"\n",
            "    println(name)\n",
            "}\n",
        ),
    );
    write_file(
        root,
        "src/main/kotlin/Engine.kt",
        "package sample\n\nclass Engine {\n    fun start() {}\n}\n",
    );
    // Android entry point: alive without any caller
    write_file(
        root,
        "src/main/kotlin/MainActivity.kt",
        "package sample\n\nclass MainActivity : Activity() {\n    override fun onCreate(state: Bundle?) {\n        super.onCreate(state)\n    }\n}\n",
    );
    // referenced only as a string literal: alive-ish (risk), must not be
    // reported dead by the default profile
    write_file(
        root,
        "src/main/kotlin/Reflected.kt",
        "package sample\n\nclass Reflected {\n    fun glow() {}\n}\n",
    );
    // dead by construction
    // "Helper", not "Service": Service-suffixed classes are retained by
    // the Android component pattern — that retention is its own test
    write_file(
        root,
        "src/main/kotlin/OrphanHelper.kt",
        "package sample\n\nclass OrphanHelper {\n    fun serve() {}\n}\n",
    );
    write_file(
        root,
        "src/main/kotlin/GhostMapper.kt",
        "package sample\n\nclass GhostMapper {\n    fun map() {}\n}\n",
    );
    write_file(
        root,
        "src/main/kotlin/Toplevel.kt",
        "package sample\n\nfun unusedTopLevel() {}\n",
    );
    // a dead symbol kept alive only by another dead symbol (zombie pair):
    // ZombieLeaf is used by GhostMapper only — both are dead
    write_file(
        root,
        "src/main/kotlin/ZombieLeaf.kt",
        "package sample\n\nclass ZombieLeaf {\n    fun rot() {}\n}\n",
    );
}

fn reported_names(out: &Output) -> Vec<String> {
    let stdout = String::from_utf8_lossy(&out.stdout);
    let json: serde_json::Value = serde_json::from_str(stdout.trim()).expect("valid JSON");
    json["issues"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|i| i["declaration"]["name"].as_str().map(str::to_string))
        .collect()
}

#[test]
fn precision_and_recall_hold_their_floors() {
    let temp = tempfile::tempdir().unwrap();
    ground_truth_project(temp.path());
    // GhostMapper references ZombieLeaf so the pair is a zombie cluster
    write_file(
        temp.path(),
        "src/main/kotlin/GhostMapper.kt",
        "package sample\n\nclass GhostMapper {\n    fun map() {\n        ZombieLeaf().rot()\n    }\n}\n",
    );

    let out = std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(temp.path())
        .args(["--format", "json"])
        .output()
        .unwrap();
    let reported = reported_names(&out);

    let true_positives = DEAD
        .iter()
        .filter(|d| reported.iter().any(|r| r == *d))
        .count();
    let false_positives = ALIVE
        .iter()
        .filter(|a| reported.iter().any(|r| r == *a))
        .count();
    let recall = true_positives as f64 / DEAD.len() as f64;

    assert_eq!(
        false_positives, 0,
        "precision floor: no living symbol may be reported dead.\nreported: {reported:?}"
    );
    assert!(
        recall >= 1.0,
        "recall floor: every known corpse is found, zombie pair included (got {true_positives}/4).\nreported: {reported:?}"
    );
}

#[test]
fn deep_mode_keeps_the_precision_floor() {
    // deep analysis reports members too — more findings, same rule:
    // nothing alive may be called dead
    let temp = tempfile::tempdir().unwrap();
    ground_truth_project(temp.path());

    let out = std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(temp.path())
        .args(["--deep", "true", "--format", "json"])
        .output()
        .unwrap();
    let reported = reported_names(&out);

    for alive in ["Engine", "start", "MainActivity"] {
        assert!(
            !reported.iter().any(|r| r == alive),
            "'{alive}' is reachable and must not be reported, reported: {reported:?}"
        );
    }
}

#[test]
fn a_zombie_symbol_names_its_real_condition() {
    // ZombieLeaf HAS an incoming reference (from dead GhostMapper);
    // "is never used" is factually wrong and sends the reader checking
    // call sites that do exist. The message must name the condition.
    let temp = tempfile::tempdir().unwrap();
    write_file(
        temp.path(),
        "src/main/kotlin/Main.kt",
        "package sample\n\nfun main() {\n    Engine().start()\n}\n",
    );
    write_file(
        temp.path(),
        "src/main/kotlin/Engine.kt",
        "package sample\n\nclass Engine {\n    fun start() {}\n}\n",
    );
    write_file(
        temp.path(),
        "src/main/kotlin/GhostMapper.kt",
        "package sample\n\nclass GhostMapper {\n    fun map() {\n        ZombieLeaf().rot()\n    }\n}\n",
    );
    write_file(
        temp.path(),
        "src/main/kotlin/ZombieLeaf.kt",
        "package sample\n\nclass ZombieLeaf {\n    fun rot() {}\n}\n",
    );

    let out = std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(temp.path())
        .args(["--format", "json"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let json: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    let zombie = json["issues"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["declaration"]["name"] == "ZombieLeaf")
        .expect("ZombieLeaf is reported");
    let message = zombie["message"].as_str().unwrap();
    assert!(
        message.contains("dead code") || message.contains("unreachable"),
        "the message names the zombie condition, was:\n{message}"
    );
    assert!(
        !message.contains("never used"),
        "'never used' is false — it IS used, by a corpse, was:\n{message}"
    );
    // the root of the cluster keeps the plain diagnosis
    let root = json["issues"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["declaration"]["name"] == "GhostMapper")
        .expect("GhostMapper is reported");
    assert!(
        root["message"].as_str().unwrap().contains("never used"),
        "a truly unreferenced symbol keeps the plain message"
    );
}

#[test]
fn java_ground_truth_holds_the_same_floors() {
    // half a typical Android repo is Java — parity is measured, not
    // assumed. Same rules: no living symbol reported, all corpses found.
    let temp = tempfile::tempdir().unwrap();
    write_file(
        temp.path(),
        "src/main/java/Main.java",
        concat!(
            "package sample;\n\n",
            "public class Main {\n",
            "    public static void main(String[] args) {\n",
            "        new JavaEngine().run();\n",
            "    }\n",
            "}\n",
        ),
    );
    write_file(
        temp.path(),
        "src/main/java/JavaEngine.java",
        "package sample;\n\npublic class JavaEngine {\n    public void run() {}\n}\n",
    );
    write_file(
        temp.path(),
        "src/main/java/JavaOrphan.java",
        "package sample;\n\npublic class JavaOrphan {\n    public void linger() {}\n}\n",
    );
    write_file(
        temp.path(),
        "src/main/java/JavaZombie.java",
        "package sample;\n\npublic class JavaZombie {\n    public void rot() {}\n}\n",
    );
    write_file(
        temp.path(),
        "src/main/java/JavaGhost.java",
        concat!(
            "package sample;\n\n",
            "public class JavaGhost {\n",
            "    public void wander() {\n",
            "        new JavaZombie().rot();\n",
            "    }\n",
            "}\n",
        ),
    );

    let out = std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(temp.path())
        .args(["--format", "json"])
        .output()
        .unwrap();
    let reported = reported_names(&out);

    for alive in ["JavaEngine", "run", "Main", "main"] {
        assert!(
            !reported.iter().any(|r| r == alive),
            "'{alive}' is reachable from main and must not be reported, got: {reported:?}"
        );
    }
    for dead in ["JavaOrphan", "JavaGhost", "JavaZombie"] {
        assert!(
            reported.iter().any(|r| r == dead),
            "'{dead}' is dead by construction and must be found, got: {reported:?}"
        );
    }
}

#[test]
fn mixed_interop_truth_holds() {
    // the cross-language edges are where parity usually dies: a Java
    // class kept alive by living Kotlin, and a Kotlin function whose
    // only caller is dead Java
    let temp = tempfile::tempdir().unwrap();
    write_file(
        temp.path(),
        "src/main/kotlin/Main.kt",
        "package sample\n\nfun main() {\n    JavaBridge().cross()\n}\n",
    );
    write_file(
        temp.path(),
        "src/main/java/JavaBridge.java",
        "package sample;\n\npublic class JavaBridge {\n    public void cross() {}\n}\n",
    );
    write_file(
        temp.path(),
        "src/main/kotlin/KotlinLeaf.kt",
        "package sample\n\nfun kotlinLeaf() {}\n",
    );
    write_file(
        temp.path(),
        "src/main/java/DeadCaller.java",
        concat!(
            "package sample;\n\n",
            "public class DeadCaller {\n",
            "    public void call() {\n",
            "        MainKt.kotlinLeaf();\n",
            "    }\n",
            "}\n",
        ),
    );

    let out = std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(temp.path())
        .args(["--format", "json"])
        .output()
        .unwrap();
    let reported = reported_names(&out);

    assert!(
        !reported.iter().any(|r| r == "JavaBridge" || r == "cross"),
        "living Kotlin keeps the Java side alive, got: {reported:?}"
    );
    assert!(
        reported.iter().any(|r| r == "DeadCaller"),
        "the dead Java caller is found, got: {reported:?}"
    );
    assert!(
        reported.iter().any(|r| r == "kotlinLeaf"),
        "a Kotlin function only dead Java calls is a zombie, got: {reported:?}"
    );
}
