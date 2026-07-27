//! Integration tests for generated-code naming conventions: DaggerXxx,
//! Xxx_Factory, Xxx_Impl, XxxDirections live in build/ and are never
//! parsed, but a reference to them proves the SOURCE class is alive.
//! Without the mapping, every class consumed only through its generated
//! artifact is a false positive.

use std::fs;
use std::path::Path;
use std::process::Output;

fn run(dir: &Path) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .output()
        .unwrap()
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn dagger_component_usage_keeps_the_interface_alive() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("AppGraph.kt"),
        "package sample\n\ninterface AppGraph {\n    fun wire()\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        concat!(
            "package sample\n\n",
            "fun main() {\n",
            "    val graph = DaggerAppGraph.builder().build()\n",
            "    graph.wire()\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("'AppGraph'"),
        "DaggerAppGraph usage proves AppGraph is alive, stdout was:\n{stdout}"
    );
}

#[test]
fn factory_usage_keeps_the_class_alive() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("PriceCatalog.kt"),
        "package sample\n\nclass PriceCatalog {\n    fun total() = 0\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Wiring.kt"),
        concat!(
            "package sample\n\n",
            "fun provide() {\n",
            "    val catalog = PriceCatalog_Factory.create()\n",
            "    println(catalog)\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("'PriceCatalog'"),
        "PriceCatalog_Factory usage proves PriceCatalog is alive, stdout was:\n{stdout}"
    );
}

#[test]
fn room_impl_usage_keeps_the_source_alive() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("OrderStore.kt"),
        "package sample\n\nabstract class OrderStore {\n    abstract fun count(): Int\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Boot.kt"),
        concat!(
            "package sample\n\n",
            "fun boot() {\n",
            "    val store = OrderStore_Impl()\n",
            "    println(store.count())\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("'OrderStore'"),
        "OrderStore_Impl usage proves OrderStore is alive, stdout was:\n{stdout}"
    );
}

#[test]
fn safeargs_directions_usage_keeps_the_screen_alive() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("CheckoutStep.kt"),
        "package sample\n\nclass CheckoutStep {\n    fun render() {}\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Nav.kt"),
        concat!(
            "package sample\n\n",
            "fun next() {\n",
            "    val action = CheckoutStepDirections.actionNext()\n",
            "    println(action)\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("'CheckoutStep'"),
        "CheckoutStepDirections usage proves CheckoutStep is alive, stdout was:\n{stdout}"
    );
}

#[test]
fn a_class_without_generated_usage_stays_dead() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("GhostRepo.kt"),
        "package sample\n\nclass GhostRepo {\n    fun haunt() {}\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("'GhostRepo'"),
        "no generated artifact in sight — still dead, stdout was:\n{stdout}"
    );
}

#[test]
fn a_real_class_shadowing_the_convention_disables_the_mapping() {
    // DaggerTool is a REAL class here, so its usage says nothing about
    // a class named Tool — mapping must only kick in when the generated
    // name has no declaration of its own.
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("DaggerTool.kt"),
        "package sample\n\nclass DaggerTool {\n    fun dig() {}\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Tool.kt"),
        "package sample\n\nclass Tool {\n    fun spin() {}\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        concat!(
            "package sample\n\n",
            "fun main() {\n",
            "    DaggerTool().dig()\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("'Tool'"),
        "a real DaggerTool class proves nothing about Tool, stdout was:\n{stdout}"
    );
}
