//! Integration tests for file-based resources: drawables, mipmaps, raw,
//! anim. Only values*.xml entries were covered — drawables are the
//! biggest typical volume of dead resources in an Android app.

use std::fs;
use std::path::Path;
use std::process::Output;

fn write_project(dir: &Path) {
    fs::create_dir_all(dir.join("res/drawable")).unwrap();
    fs::create_dir_all(dir.join("res/mipmap-hdpi")).unwrap();
    fs::create_dir_all(dir.join("res/raw")).unwrap();
    fs::write(dir.join("res/drawable/used_icon.png"), b"png").unwrap();
    fs::write(dir.join("res/drawable/dead_icon.png"), b"png").unwrap();
    fs::write(dir.join("res/drawable/xml_used_bg.png"), b"png").unwrap();
    fs::write(dir.join("res/drawable/selector.xml"), "<selector xmlns:android=\"http://schemas.android.com/apk/res/android\">\n    <item android:drawable=\"@drawable/xml_used_bg\" />\n</selector>\n").unwrap();
    fs::write(dir.join("res/mipmap-hdpi/ic_launcher.png"), b"png").unwrap();
    fs::write(dir.join("res/raw/dead_sound.ogg"), b"ogg").unwrap();
    fs::write(
        dir.join("AndroidManifest.xml"),
        concat!(
            "<manifest xmlns:android=\"http://schemas.android.com/apk/res/android\">\n",
            "    <application android:icon=\"@mipmap/ic_launcher\" />\n",
            "</manifest>\n",
        ),
    )
    .unwrap();
    fs::write(
        dir.join("Main.kt"),
        concat!(
            "package sample\n\n",
            "fun main() {\n",
            "    println(R.drawable.used_icon)\n",
            "    println(R.drawable.selector)\n",
            "}\n",
        ),
    )
    .unwrap();
}

fn run(dir: &Path) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .arg("--unused-resources")
        .arg("--format")
        .arg("json")
        .output()
        .unwrap()
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn a_dead_drawable_is_reported() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("dead_icon"),
        "nothing references dead_icon.png, stdout was:\n{stdout}"
    );
}

#[test]
fn a_code_referenced_drawable_stays_silent() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("used_icon"),
        "R.drawable.used_icon is a reference, stdout was:\n{stdout}"
    );
}

#[test]
fn an_xml_referenced_drawable_stays_silent() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("xml_used_bg"),
        "@drawable/xml_used_bg in the selector is a reference, stdout was:\n{stdout}"
    );
}

#[test]
fn the_launcher_mipmap_referenced_by_the_manifest_stays_silent() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("ic_launcher"),
        "@mipmap/ic_launcher lives in the manifest, stdout was:\n{stdout}"
    );
}

#[test]
fn a_dead_raw_file_is_reported() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("dead_sound"),
        "nothing plays dead_sound.ogg, stdout was:\n{stdout}"
    );
}

#[test]
fn density_variants_are_one_resource() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());
    fs::create_dir_all(temp.path().join("res/drawable-hdpi")).unwrap();
    fs::create_dir_all(temp.path().join("res/drawable-xhdpi")).unwrap();
    fs::write(temp.path().join("res/drawable-hdpi/multi_dead.png"), b"png").unwrap();
    fs::write(
        temp.path().join("res/drawable-xhdpi/multi_dead.png"),
        b"png",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    let findings = stdout.matches("\"name\": \"multi_dead\"").count();
    assert_eq!(
        findings, 1,
        "densities collapse to one resource, not one finding per density, stdout was:\n{stdout}"
    );
}

#[test]
fn a_qualified_dir_resource_referenced_in_code_stays_silent() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());
    fs::create_dir_all(temp.path().join("res/drawable-night")).unwrap();
    fs::write(temp.path().join("res/drawable-night/night_bg.png"), b"png").unwrap();
    fs::write(
        temp.path().join("Night.kt"),
        "package sample\n\nfun night() {\n    println(R.drawable.night_bg)\n}\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("night_bg"),
        "drawable-night is still R.drawable, stdout was:\n{stdout}"
    );
}

#[test]
fn nine_patch_names_lose_their_suffix() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());
    fs::write(temp.path().join("res/drawable/patched_bg.9.png"), b"png").unwrap();
    fs::write(
        temp.path().join("Patch.kt"),
        "package sample\n\nfun patch() {\n    println(R.drawable.patched_bg)\n}\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("patched_bg"),
        "patched_bg.9.png is R.drawable.patched_bg, stdout was:\n{stdout}"
    );
}
