//! Integration tests for event-bus orphan detection.
//!
//! An event posted with no subscriber is a message into the void; a
//! @Subscribe handler for an event never posted is dead weight. Dynamic
//! posts (bus.post(variable)) make the second verdict uncertain, so it
//! carries an explicit caveat when they exist.

use std::fs;
use std::path::Path;
use std::process::Output;

fn write_bus_project(dir: &Path) {
    fs::write(
        dir.join("Events.kt"),
        concat!(
            "package sample\n\n",
            "class HandledEvent\n",
            "class OrphanEvent\n",
            "class GhostEvent\n",
        ),
    )
    .unwrap();
    fs::write(
        dir.join("Main.kt"),
        concat!(
            "package sample\n\n",
            "fun main() {\n",
            "    Bus.post(HandledEvent())\n",
            "    Bus.post(OrphanEvent())\n",
            "    Listener().register()\n",
            "}\n\n",
            "object Bus {\n",
            "    fun post(event: Any) {}\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        dir.join("Listener.kt"),
        concat!(
            "package sample\n\n",
            "class Listener {\n",
            "    fun register() {}\n\n",
            "    @Subscribe\n",
            "    fun onHandled(event: HandledEvent) {}\n\n",
            "    @Subscribe\n",
            "    fun onGhost(event: GhostEvent) {}\n",
            "}\n",
        ),
    )
    .unwrap();
}

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
fn a_posted_event_nobody_hears_is_reported() {
    let temp = tempfile::tempdir().unwrap();
    write_bus_project(temp.path());

    let output = run(temp.path());

    let stdout = stdout_of(&output);
    assert!(
        stdout.contains("posted but never subscribed"),
        "the section names itself, stdout was:\n{stdout}"
    );
    let section = stdout
        .split("posted but never subscribed")
        .nth(1)
        .unwrap_or("");
    assert!(
        section.contains("OrphanEvent"),
        "OrphanEvent goes into the void, stdout was:\n{stdout}"
    );
}

#[test]
fn a_subscriber_of_a_never_posted_event_is_reported() {
    let temp = tempfile::tempdir().unwrap();
    write_bus_project(temp.path());

    let output = run(temp.path());

    let stdout = stdout_of(&output);
    assert!(
        stdout.contains("subscribed but never posted"),
        "the second section exists, stdout was:\n{stdout}"
    );
    let section = stdout
        .split("subscribed but never posted")
        .nth(1)
        .unwrap_or("");
    assert!(
        section.contains("GhostEvent"),
        "GhostEvent has a listener but no sender, stdout was:\n{stdout}"
    );
}

#[test]
fn a_matched_pair_stays_silent() {
    let temp = tempfile::tempdir().unwrap();
    write_bus_project(temp.path());

    let output = run(temp.path());

    let stdout = stdout_of(&output);
    let bus_part = stdout.split("bus").nth(1).unwrap_or(&stdout);
    assert!(
        !bus_part.contains("HandledEvent"),
        "posted AND subscribed = healthy, stdout was:\n{stdout}"
    );
}

#[test]
fn dynamic_posts_add_an_honest_caveat() {
    let temp = tempfile::tempdir().unwrap();
    write_bus_project(temp.path());
    fs::write(
        temp.path().join("Dynamic.kt"),
        "package sample\n\nfun relay(pending: Any) {\n    Bus.post(pending)\n}\n",
    )
    .unwrap();

    let output = run(temp.path());

    let stdout = stdout_of(&output);
    assert!(
        stdout.contains("dynamic post"),
        "uncertainty is stated when posts cannot be enumerated, stdout was:\n{stdout}"
    );
}

#[test]
fn java_subscriber_syntax_is_understood() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Events.kt"),
        "package sample\n\nclass JavaGhostEvent\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("JListener.java"),
        concat!(
            "package sample;\n\n",
            "public class JListener {\n",
            "    @Subscribe\n",
            "    public void onGhost(JavaGhostEvent event) {}\n",
            "}\n",
        ),
    )
    .unwrap();

    let output = run(temp.path());

    let stdout = stdout_of(&output);
    assert!(
        stdout.contains("JavaGhostEvent"),
        "Java handler syntax is parsed too, stdout was:\n{stdout}"
    );
}
