//! Structural guard for the GitHub Action: action.yml must stay a
//! valid composite action with the documented inputs — a YAML typo
//! there only ever explodes in someone else's CI.

use serde_yaml_bw::Value;

fn action() -> Value {
    let raw = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("action.yml"),
    )
    .expect("action.yml exists at the repo root");
    serde_yaml_bw::from_str(&raw).expect("action.yml parses as YAML")
}

fn all_run_steps() -> String {
    let action = action();
    action["runs"]["steps"]
        .as_sequence()
        .expect("steps list")
        .iter()
        .filter_map(|s| s["run"].as_str().map(str::to_string))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn the_action_is_a_composite_with_shelled_steps() {
    let action = action();
    assert_eq!(action["runs"]["using"], "composite");
    let steps = action["runs"]["steps"].as_sequence().expect("steps list");
    assert!(steps.len() >= 2, "at least install + run steps");
    for step in steps {
        let has_shell = step["shell"].as_str().is_some();
        let is_uses = step["uses"].as_str().is_some();
        assert!(
            has_shell || is_uses,
            "composite run steps must declare a shell, step was:\n{step:?}"
        );
    }
}

#[test]
fn the_documented_inputs_exist() {
    let action = action();
    for input in [
        "path",
        "format",
        "output",
        "args",
        "version",
        "fail-on-findings",
        "min-confidence",
    ] {
        assert!(
            !action["inputs"][input].is_null(),
            "input '{input}' is part of the contract"
        );
    }
    assert_eq!(action["inputs"]["path"]["default"], ".");
}

#[test]
fn the_run_steps_wire_the_inputs_through() {
    let run_all = all_run_steps();
    for placeholder in [
        "inputs.path",
        "inputs.format",
        "inputs.output",
        "inputs.args",
    ] {
        assert!(
            run_all.contains(placeholder),
            "'{placeholder}' must reach the CLI, steps were:\n{run_all}"
        );
    }
}

#[test]
fn the_json_findings_count_reads_the_findings_array() {
    // the JSON report is {"findings": [...], "summary": {...}} — a bare
    // `jq 'length'` counts the two object keys and always answers 2
    let run_all = all_run_steps();
    assert!(
        run_all.contains(".findings | length"),
        "the count must target the findings array, steps were:\n{run_all}"
    );
}
