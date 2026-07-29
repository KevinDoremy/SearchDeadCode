//! Cas réel : une propriété sortait en "assigned 2 time(s) but never
//! read" alors qu'elle est lue dans des logs via interpolation
//! ("... $pendingUid"). La grammaire aliase
//! l'identifiant en interpolated_identifier ($x) ou en
//! interpolated_expression (${x} nu) — aucun des deux ne matche le bras
//! simple_identifier du walker, la lecture était invisible.

use std::fs;
use std::path::Path;
use std::process::Output;

fn run(dir: &Path) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .arg("--deep")
        .output()
        .unwrap()
}

fn write_fixture(dir: &Path, interpolation: &str) {
    fs::write(
        dir.join("Session.kt"),
        format!(
            concat!(
                "package sample\n\n",
                "class Session {{\n",
                "    private var pendingUid: String? = null\n\n",
                "    fun schedule(uid: String) {{\n",
                "        pendingUid = uid\n",
                "        println(\"pending: {}\")\n",
                "    }}\n\n",
                "    fun clear() {{\n",
                "        pendingUid = null\n",
                "    }}\n",
                "}}\n\n",
                "fun main() {{\n",
                "    val s = Session()\n",
                "    s.schedule(\"abc\")\n",
                "    s.clear()\n",
                "}}\n",
            ),
            interpolation
        ),
    )
    .unwrap();
}

#[test]
fn a_property_read_through_dollar_interpolation_is_not_assign_only() {
    let temp = tempfile::tempdir().unwrap();
    write_fixture(temp.path(), "$pendingUid");
    let stdout = String::from_utf8_lossy(&run(temp.path()).stdout).to_string();
    assert!(
        !stdout.contains("pendingUid"),
        "une lecture via $x dans un template est une lecture, stdout:\n{stdout}"
    );
}

#[test]
fn a_property_read_through_braced_interpolation_is_not_assign_only() {
    let temp = tempfile::tempdir().unwrap();
    write_fixture(temp.path(), "${pendingUid}");
    let stdout = String::from_utf8_lossy(&run(temp.path()).stdout).to_string();
    assert!(
        !stdout.contains("pendingUid"),
        "une lecture via ${{x}} dans un template est une lecture, stdout:\n{stdout}"
    );
}
