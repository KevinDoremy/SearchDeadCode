//! Integration tests for --middlemen: a class whose every method just
//! forwards to the same delegate is a post-migration leftover — the
//! callers can talk to the delegate directly.

use std::fs;
use std::path::Path;
use std::process::Output;

fn run(dir: &Path) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .arg("--middlemen")
        .output()
        .unwrap()
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn engine_kt(dir: &Path) {
    fs::write(
        dir.join("OrderEngine.kt"),
        concat!(
            "package sample\n\n",
            "class OrderEngine {\n",
            "    fun place(order: String) {}\n",
            "    fun cancel(id: Int) {}\n",
            "}\n",
        ),
    )
    .unwrap();
}

#[test]
fn a_pure_delegating_class_is_flagged() {
    let temp = tempfile::tempdir().unwrap();
    engine_kt(temp.path());
    fs::write(
        temp.path().join("OrderFacade.kt"),
        concat!(
            "package sample\n\n",
            "class OrderFacade(private val engine: OrderEngine) {\n",
            "    fun place(order: String) = engine.place(order)\n",
            "    fun cancel(id: Int) = engine.cancel(id)\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        concat!(
            "package sample\n\n",
            "fun main() {\n",
            "    val facade = OrderFacade(OrderEngine())\n",
            "    facade.place(\"a\")\n",
            "    facade.cancel(1)\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("OrderFacade") && stdout.contains("engine"),
        "the pure forwarder and its delegate are named, stdout was:\n{stdout}"
    );
}

#[test]
fn a_class_with_real_logic_is_not_flagged() {
    let temp = tempfile::tempdir().unwrap();
    engine_kt(temp.path());
    fs::write(
        temp.path().join("OrderService.kt"),
        concat!(
            "package sample\n\n",
            "class OrderService(private val engine: OrderEngine) {\n",
            "    fun place(order: String) = engine.place(order)\n",
            "    fun cancelAll(ids: List<Int>) {\n",
            "        for (id in ids) {\n",
            "            engine.cancel(id)\n",
            "        }\n",
            "    }\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        concat!(
            "package sample\n\n",
            "fun main() {\n",
            "    val service = OrderService(OrderEngine())\n",
            "    service.place(\"a\")\n",
            "    service.cancelAll(listOf(1))\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("OrderService"),
        "a loop is logic, not forwarding, stdout was:\n{stdout}"
    );
}

#[test]
fn a_single_method_class_is_not_flagged() {
    let temp = tempfile::tempdir().unwrap();
    engine_kt(temp.path());
    fs::write(
        temp.path().join("Thin.kt"),
        concat!(
            "package sample\n\n",
            "class Thin(private val engine: OrderEngine) {\n",
            "    fun place(order: String) = engine.place(order)\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        concat!(
            "package sample\n\n",
            "fun main() {\n",
            "    Thin(OrderEngine()).place(\"a\")\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("Thin"),
        "one method is not enough evidence, stdout was:\n{stdout}"
    );
}

#[test]
fn mixed_receivers_are_not_flagged() {
    let temp = tempfile::tempdir().unwrap();
    engine_kt(temp.path());
    fs::write(
        temp.path().join("Splitter.kt"),
        concat!(
            "package sample\n\n",
            "class Splitter(private val left: OrderEngine, private val right: OrderEngine) {\n",
            "    fun place(order: String) = left.place(order)\n",
            "    fun cancel(id: Int) = right.cancel(id)\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        concat!(
            "package sample\n\n",
            "fun main() {\n",
            "    val s = Splitter(OrderEngine(), OrderEngine())\n",
            "    s.place(\"a\")\n",
            "    s.cancel(1)\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("Splitter"),
        "routing between two delegates is a decision, stdout was:\n{stdout}"
    );
}

#[test]
fn no_middlemen_is_a_clean_answer() {
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
        "no middlemen is fine, output was:\n{output:?}"
    );
    assert!(
        stdout.to_lowercase().contains("no middleman"),
        "the verdict is explicit, stdout was:\n{stdout}"
    );
}
