//! Integration tests for --unused-deps: a dependency declared in
//! build.gradle(.kts) that no source file ever imports is dead weight
//! on every build — the complement of dead code.

use std::fs;
use std::path::Path;
use std::process::Output;

fn run(dir: &Path) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .arg("--unused-deps")
        .output()
        .unwrap()
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn a_declared_dependency_never_imported_is_flagged() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("build.gradle.kts"),
        concat!(
            "dependencies {\n",
            "    implementation(\"com.squareup.moshi:moshi:1.15.0\")\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("com.squareup.moshi:moshi"),
        "the never-imported dependency is flagged, stdout was:\n{stdout}"
    );
}

#[test]
fn an_imported_dependency_is_not_flagged() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("build.gradle.kts"),
        concat!(
            "dependencies {\n",
            "    implementation(\"com.squareup.retrofit2:retrofit:2.11.0\")\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Api.kt"),
        concat!(
            "package sample\n\n",
            "import retrofit2.Retrofit\n\n",
            "fun build(): Retrofit = Retrofit.Builder().build()\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("com.squareup.retrofit2:retrofit"),
        "an imported dependency is used, stdout was:\n{stdout}"
    );
}

#[test]
fn a_group_alias_counts_as_usage() {
    // gson's group is com.google.code.gson but the package is
    // com.google.gson — the last group segment appearing as a package
    // segment is the usage signal
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("build.gradle.kts"),
        concat!(
            "dependencies {\n",
            "    implementation(\"com.google.code.gson:gson:2.11.0\")\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Json.kt"),
        concat!(
            "package sample\n\n",
            "import com.google.gson.Gson\n\n",
            "fun parser() = Gson()\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("com.google.code.gson:gson"),
        "gson is imported under its real package, stdout was:\n{stdout}"
    );
}

#[test]
fn project_platform_and_processor_deps_are_ignored() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("build.gradle.kts"),
        concat!(
            "dependencies {\n",
            "    implementation(project(\":core\"))\n",
            "    implementation(platform(\"androidx.compose:compose-bom:2024.06.00\"))\n",
            "    ksp(\"com.google.dagger:dagger-compiler:2.51\")\n",
            "    kapt(\"com.github.bumptech.glide:compiler:4.16.0\")\n",
            "    runtimeOnly(\"org.postgresql:postgresql:42.7.0\")\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {}\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains(":core")
            && !stdout.contains("compose-bom")
            && !stdout.contains("dagger-compiler")
            && !stdout.contains("glide")
            && !stdout.contains("postgresql"),
        "project/platform/processor/runtime deps are out of scope, stdout was:\n{stdout}"
    );
}

#[test]
fn version_catalog_refs_are_skipped_not_crashed() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("build.gradle.kts"),
        concat!(
            "dependencies {\n",
            "    implementation(libs.moshi)\n",
            "    implementation(\"com.squareup.okio:okio:3.9.0\")\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {}\n",
    )
    .unwrap();

    let output = run(temp.path());
    let stdout = stdout_of(&output);
    assert!(
        output.status.success(),
        "catalog refs don't break parsing, output was:\n{output:?}"
    );
    assert!(
        stdout.contains("com.squareup.okio:okio"),
        "string coordinates next to catalog refs still work, stdout was:\n{stdout}"
    );
}

#[test]
fn groovy_single_quotes_work_too() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("build.gradle"),
        concat!(
            "dependencies {\n",
            "    implementation 'io.reactivex.rxjava3:rxjava:3.1.8'\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {}\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("io.reactivex.rxjava3:rxjava"),
        "groovy quote style parses, stdout was:\n{stdout}"
    );
}

#[test]
fn commented_out_declarations_do_not_count() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("build.gradle.kts"),
        concat!(
            "dependencies {\n",
            "    // implementation(\"com.example.ghost:ghost:1.0\")\n",
            "    implementation(\"com.squareup.okio:okio:3.9.0\")\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {}\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("com.example.ghost"),
        "a commented-out dependency is not declared, stdout was:\n{stdout}"
    );
    assert!(
        stdout.contains("com.squareup.okio:okio"),
        "the live declaration on the next line still counts, stdout was:\n{stdout}"
    );
}

#[test]
fn a_wildcard_import_counts_as_usage() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("build.gradle.kts"),
        concat!(
            "dependencies {\n",
            "    implementation(\"com.squareup.retrofit2:retrofit:2.11.0\")\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Api.kt"),
        "package sample\n\nimport retrofit2.*\n\nfun noop() {}\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("com.squareup.retrofit2:retrofit"),
        "a star import is still an import, stdout was:\n{stdout}"
    );
}

#[test]
fn a_java_import_with_semicolon_counts_as_usage() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("build.gradle"),
        concat!(
            "dependencies {\n",
            "    implementation 'com.google.code.gson:gson:2.11.0'\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Json.java"),
        concat!(
            "package sample;\n\n",
            "import com.google.gson.Gson;\n\n",
            "public class Json {\n    Gson gson = new Gson();\n}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("com.google.code.gson:gson"),
        "java import syntax is understood, stdout was:\n{stdout}"
    );
}

#[test]
fn no_gradle_files_is_a_clean_answer() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {}\n",
    )
    .unwrap();

    let output = run(temp.path());
    let stdout = stdout_of(&output);
    assert!(
        output.status.success(),
        "no gradle files is not an error, output was:\n{output:?}"
    );
    assert!(
        stdout.to_lowercase().contains("no gradle build files"),
        "the verdict is explicit, stdout was:\n{stdout}"
    );
}
