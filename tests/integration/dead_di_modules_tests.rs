//! Integration tests for --dead-di-modules: a @Module whose every
//! binding produces a type nobody consumes is a whole DI cluster to
//! delete — the @Module annotation retains it, so the standard report
//! never says so.

use std::fs;
use std::path::Path;
use std::process::Output;

fn run(dir: &Path) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .arg("--dead-di-modules")
        .output()
        .unwrap()
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn a_module_with_no_consumed_binding_is_flagged() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("GhostModule.kt"),
        concat!(
            "package sample\n\n",
            "@Module\n",
            "class GhostModule {\n",
            "    @Provides\n",
            "    fun provideRelic(): RelicStore = RelicStore()\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("RelicStore.kt"),
        "package sample\n\nclass RelicStore {\n    fun dust() {}\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("GhostModule"),
        "nobody consumes RelicStore, the module is dead weight, stdout was:\n{stdout}"
    );
}

#[test]
fn a_module_with_a_consumed_binding_is_not_flagged() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("StoreModule.kt"),
        concat!(
            "package sample\n\n",
            "@Module\n",
            "class StoreModule {\n",
            "    @Provides\n",
            "    fun provideStore(): RelicStore = RelicStore()\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("RelicStore.kt"),
        "package sample\n\nclass RelicStore {\n    fun dust() {}\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        concat!(
            "package sample\n\n",
            "fun main() {\n",
            "    val store: RelicStore = fetch()\n",
            "    store.dust()\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("StoreModule"),
        "one consumed binding keeps the module, stdout was:\n{stdout}"
    );
}

#[test]
fn a_class_without_module_annotation_is_ignored() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("PlainFactory.kt"),
        concat!(
            "package sample\n\n",
            "class PlainFactory {\n",
            "    fun makeRelic(): RelicStore = RelicStore()\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("RelicStore.kt"),
        "package sample\n\nclass RelicStore {\n    fun dust() {}\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("PlainFactory"),
        "only DI modules are in scope here, stdout was:\n{stdout}"
    );
}

#[test]
fn no_di_modules_is_a_clean_answer() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();

    let output = run(temp.path());
    let stdout = stdout_of(&output);
    assert!(
        output.status.success(),
        "no DI modules is fine, output was:\n{output:?}"
    );
    assert!(
        stdout.to_lowercase().contains("no dead di modules"),
        "the verdict is explicit, stdout was:\n{stdout}"
    );
}
