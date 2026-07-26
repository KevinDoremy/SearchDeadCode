//! Integration tests for --init: generate a commented .deadcode.yml that
//! matches the project's real shape (source sets, DI framework, exclusions).

use std::fs;
use std::path::Path;
use std::process::Output;

fn write_sample_project(dir: &Path) {
    fs::write(
        dir.join("build.gradle.kts"),
        "plugins { kotlin(\"jvm\") }\n",
    )
    .unwrap();
    let main = dir.join("src/main/kotlin");
    fs::create_dir_all(&main).unwrap();
    fs::write(main.join("Main.kt"), "package app\n\nfun main() {}\n").unwrap();
    fs::write(
        main.join("TrackerModule.kt"),
        "package app\n\n@Module\nclass TrackerModule {\n    @Provides\n    fun provide(): String = \"x\"\n}\n",
    )
    .unwrap();
    let phantom = dir.join("src/savedTests/kotlin");
    fs::create_dir_all(&phantom).unwrap();
    fs::write(phantom.join("Old.kt"), "package app\n\nclass Old\n").unwrap();
}

fn run_init(dir: &Path) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .arg("--init")
        .output()
        .unwrap()
}

#[test]
fn init_writes_a_commented_config_matching_the_project() {
    let temp = tempfile::tempdir().unwrap();
    write_sample_project(temp.path());

    run_init(temp.path());

    let config_path = temp.path().join(".deadcode.yml");
    assert!(config_path.exists(), "--init writes .deadcode.yml");
    let content = fs::read_to_string(&config_path).unwrap();
    assert!(
        content.contains("exclude"),
        "the config has an exclude section, content was:\n{content}"
    );
    assert!(
        content.contains("savedTests"),
        "phantom source sets are excluded up front, content was:\n{content}"
    );
    assert!(
        content.contains('#'),
        "the generated config is commented for humans, content was:\n{content}"
    );
}

#[test]
fn init_names_the_detected_di_framework() {
    let temp = tempfile::tempdir().unwrap();
    write_sample_project(temp.path());

    run_init(temp.path());

    let content = fs::read_to_string(temp.path().join(".deadcode.yml")).unwrap();
    assert!(
        content.contains("Dagger"),
        "@Module/@Provides usage is detected and named, content was:\n{content}"
    );
}

#[test]
fn init_refuses_to_overwrite_an_existing_config() {
    let temp = tempfile::tempdir().unwrap();
    write_sample_project(temp.path());
    fs::write(temp.path().join(".deadcode.yml"), "targets: [custom]\n").unwrap();

    let output = run_init(temp.path());

    let content = fs::read_to_string(temp.path().join(".deadcode.yml")).unwrap();
    assert_eq!(
        content, "targets: [custom]\n",
        "an existing config is never clobbered"
    );
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        combined.contains("already exists"),
        "the refusal is explicit, output was:\n{combined}"
    );
}
