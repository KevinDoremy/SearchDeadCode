//! `--redundant-public false` and `--redundant-null-init false`: the two style
//! rules a dead-code sweep wants out of the way, switchable from the command
//! line and not only from `.deadcode.yml`.

use std::fs;
use std::path::Path;

fn codes(dir: &Path, extra: &[&str]) -> Vec<String> {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .args(["--format", "json", "--incremental=false"])
        .args(extra)
        .output()
        .unwrap();
    let out = String::from_utf8_lossy(&output.stdout).to_string();
    let start = out.find('{').unwrap_or(0);
    let parsed: serde_json::Value = serde_json::from_str(&out[start..]).unwrap();
    parsed["issues"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["code"].as_str().unwrap_or("").to_string())
        .collect()
}

fn corpus(dir: &Path) {
    fs::write(
        dir.join("Main.kt"),
        "package s\n\nfun main() {\n    println(Holder().value())\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("Holder.java"),
        "package s;\n\npublic class Holder {\n    private Object cache = null;\n\n    public Object value() {\n        if (cache == null) {\n            cache = new Object();\n        }\n        return cache;\n    }\n}\n",
    )
    .unwrap();
}

#[test]
fn style_rules_can_be_switched_off_from_the_command_line() {
    let temp = tempfile::tempdir().unwrap();
    corpus(temp.path());

    let default_run = codes(temp.path(), &[]);
    assert!(
        default_run.iter().any(|c| c == "DC013"),
        "the explicit `= null` is reported by default: {default_run:?}"
    );

    let switched_off = codes(
        temp.path(),
        &[
            "--redundant-public",
            "false",
            "--redundant-null-init",
            "false",
        ],
    );
    assert!(
        !switched_off.iter().any(|c| c == "DC013" || c == "DC006"),
        "both style rules are off: {switched_off:?}"
    );
}
