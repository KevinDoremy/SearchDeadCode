//! Integration tests for --unused-assets: files under assets/ are read
//! by string path (assets.open, createFromAsset, android_asset URLs) —
//! an asset whose path or name appears nowhere ships dead bytes in
//! every APK.

use std::fs;
use std::path::Path;
use std::process::Output;

fn run(dir: &Path) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .arg("--unused-assets")
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

#[test]
fn an_asset_nobody_reads_is_flagged() {
    let temp = tempfile::tempdir().unwrap();
    write_file(temp.path(), "src/main/assets/data/legacy.json", "{}");
    write_file(
        temp.path(),
        "src/main/kotlin/Main.kt",
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    );

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("legacy.json"),
        "the unread asset is flagged, stdout was:\n{stdout}"
    );
}

#[test]
fn an_asset_opened_by_path_is_not_flagged() {
    let temp = tempfile::tempdir().unwrap();
    write_file(temp.path(), "src/main/assets/data/config.json", "{}");
    write_file(
        temp.path(),
        "src/main/kotlin/Main.kt",
        concat!(
            "package sample\n\n",
            "fun load(am: AssetManager) {\n",
            "    am.open(\"data/config.json\")\n",
            "}\n",
        ),
    );

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("config.json"),
        "an opened asset is alive, stdout was:\n{stdout}"
    );
}

#[test]
fn a_basename_mention_counts_as_usage() {
    // paths are often built dynamically: "data/" + name
    let temp = tempfile::tempdir().unwrap();
    write_file(temp.path(), "src/main/assets/fonts/brand.ttf", "");
    write_file(
        temp.path(),
        "src/main/kotlin/Fonts.kt",
        concat!(
            "package sample\n\n",
            "fun font(am: AssetManager, dir: String) {\n",
            "    Typeface.createFromAsset(am, dir + \"brand.ttf\")\n",
            "}\n",
        ),
    );

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("brand.ttf"),
        "a basename mention is enough to stay conservative, stdout was:\n{stdout}"
    );
}

#[test]
fn cross_asset_references_count() {
    // web assets reference each other: the html keeps the css alive
    let temp = tempfile::tempdir().unwrap();
    write_file(
        temp.path(),
        "src/main/assets/web/page.html",
        "<link rel=\"stylesheet\" href=\"style.css\">",
    );
    write_file(temp.path(), "src/main/assets/web/style.css", "body {}");
    write_file(
        temp.path(),
        "src/main/kotlin/Main.kt",
        concat!(
            "package sample\n\n",
            "fun show(view: WebView) {\n",
            "    view.loadUrl(\"file:///android_asset/web/page.html\")\n",
            "}\n",
        ),
    );

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("style.css"),
        "the html reference keeps the css alive, stdout was:\n{stdout}"
    );
    assert!(
        !stdout.contains("page.html"),
        "the loadUrl reference keeps the page alive, stdout was:\n{stdout}"
    );
}

#[test]
fn self_mention_does_not_keep_an_asset_alive() {
    let temp = tempfile::tempdir().unwrap();
    write_file(
        temp.path(),
        "src/main/assets/manifest.json",
        "{ \"self\": \"manifest.json\" }",
    );
    write_file(
        temp.path(),
        "src/main/kotlin/Main.kt",
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    );

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("manifest.json"),
        "a file naming itself proves nothing, stdout was:\n{stdout}"
    );
}

#[test]
fn no_assets_dir_is_a_clean_answer() {
    let temp = tempfile::tempdir().unwrap();
    write_file(
        temp.path(),
        "src/main/kotlin/Main.kt",
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    );

    let output = run(temp.path());
    let stdout = stdout_of(&output);
    assert!(
        output.status.success(),
        "no assets dir is fine, output was:\n{output:?}"
    );
    assert!(
        stdout.to_lowercase().contains("no assets"),
        "the verdict is explicit, stdout was:\n{stdout}"
    );
}
