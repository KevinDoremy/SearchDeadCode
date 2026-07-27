//! Integration tests for --unused-permissions: a manifest permission
//! whose corresponding API family never appears in the code is pure
//! liability — privacy review friction and Play Store questions for
//! a capability nobody uses.

use std::fs;
use std::path::Path;
use std::process::Output;

fn run(dir: &Path) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .arg("--unused-permissions")
        .output()
        .unwrap()
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn write_file(root: &Path, rel: &str, content: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

fn manifest(root: &Path, permissions: &[&str]) {
    let mut body = String::from(
        "<manifest xmlns:android=\"http://schemas.android.com/apk/res/android\" package=\"sample\">\n",
    );
    for permission in permissions {
        body.push_str(&format!(
            "    <uses-permission android:name=\"{permission}\"/>\n"
        ));
    }
    body.push_str("</manifest>\n");
    write_file(root, "src/main/AndroidManifest.xml", &body);
}

#[test]
fn an_unused_camera_permission_is_flagged() {
    let temp = tempfile::tempdir().unwrap();
    manifest(temp.path(), &["android.permission.CAMERA"]);
    write_file(
        temp.path(),
        "src/main/kotlin/Main.kt",
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    );

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("android.permission.CAMERA"),
        "no camera API in sight, the permission is flagged, stdout was:\n{stdout}"
    );
}

#[test]
fn a_used_camera_permission_is_not_flagged() {
    let temp = tempfile::tempdir().unwrap();
    manifest(temp.path(), &["android.permission.CAMERA"]);
    write_file(
        temp.path(),
        "src/main/kotlin/Capture.kt",
        concat!(
            "package sample\n\n",
            "fun open(manager: CameraManager) {\n",
            "    manager.openCamera(\"0\", callback, null)\n",
            "}\n\n",
            "fun main() {\n    println(\"alive\")\n}\n",
        ),
    );

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("android.permission.CAMERA"),
        "CameraManager proves the capability is used, stdout was:\n{stdout}"
    );
}

#[test]
fn a_runtime_permission_check_counts_as_usage() {
    let temp = tempfile::tempdir().unwrap();
    manifest(temp.path(), &["android.permission.RECORD_AUDIO"]);
    write_file(
        temp.path(),
        "src/main/kotlin/Gate.kt",
        concat!(
            "package sample\n\n",
            "fun allowed(ctx: Context): Boolean {\n",
            "    return ctx.checkSelfPermission(Manifest.permission.RECORD_AUDIO) == GRANTED\n",
            "}\n\n",
            "fun main() {\n    println(\"alive\")\n}\n",
        ),
    );

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("RECORD_AUDIO"),
        "an explicit permission check is intent enough, stdout was:\n{stdout}"
    );
}

#[test]
fn an_unknown_permission_is_left_alone() {
    let temp = tempfile::tempdir().unwrap();
    manifest(temp.path(), &["com.example.custom.MAGIC_HANDSHAKE"]);
    write_file(
        temp.path(),
        "src/main/kotlin/Main.kt",
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    );

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("MAGIC_HANDSHAKE"),
        "unknown permissions are unverifiable, stdout was:\n{stdout}"
    );
}

#[test]
fn the_same_permission_in_two_manifests_is_reported_once() {
    let temp = tempfile::tempdir().unwrap();
    manifest(temp.path(), &["android.permission.VIBRATE"]);
    write_file(
        temp.path(),
        "src/debug/AndroidManifest.xml",
        concat!(
            "<manifest xmlns:android=\"http://schemas.android.com/apk/res/android\" package=\"sample\">\n",
            "    <uses-permission android:name=\"android.permission.VIBRATE\"/>\n",
            "</manifest>\n",
        ),
    );
    write_file(
        temp.path(),
        "src/main/kotlin/Main.kt",
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    );

    let stdout = stdout_of(&run(temp.path()));
    let count = stdout.matches("android.permission.VIBRATE").count();
    assert_eq!(count, 1, "one permission, one line, stdout was:\n{stdout}");
}

#[test]
fn no_manifest_is_a_clean_answer() {
    let temp = tempfile::tempdir().unwrap();
    write_file(
        temp.path(),
        "Main.kt",
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    );

    let output = run(temp.path());
    let stdout = stdout_of(&output);
    assert!(
        output.status.success(),
        "no manifest is fine, output was:\n{output:?}"
    );
    assert!(
        stdout.to_lowercase().contains("no manifest"),
        "the verdict is explicit, stdout was:\n{stdout}"
    );
}
