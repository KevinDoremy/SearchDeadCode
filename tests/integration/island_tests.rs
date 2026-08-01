//! Integration tests for --islands: groups of declarations that reference
//! only each other and are referenced by nothing else.
//!
//! The error direction under test is the model's whole point: everything
//! that cannot be placed roots (XML tokens, string literals in live code,
//! guarded declarations' contents), while a string literal INSIDE the island
//! itself must not resurrect it, and one external caller dissolves the
//! island whole — never partially.

use std::fs;
use std::path::Path;
use std::process::Output;

fn run(dir: &Path) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .arg("--islands")
        .arg("--quiet")
        .output()
        .unwrap()
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn write_ring(dir: &Path) {
    fs::write(
        dir.join("Ring.kt"),
        concat!(
            "package sample\n\n",
            "fun ringA() {\n    ringB()\n}\n\n",
            "fun ringB() {\n    ringA()\n}\n",
        ),
    )
    .unwrap();
    fs::write(
        dir.join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"app\")\n}\n",
    )
    .unwrap();
}

#[test]
fn a_mutual_pair_is_an_island_with_its_chain() {
    let temp = tempfile::tempdir().unwrap();
    write_ring(temp.path());

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("ringA") && stdout.contains("ringB"),
        "two functions holding only each other are one island, stdout was:\n{stdout}"
    );
    assert!(
        stdout.contains("kept alive only by"),
        "the report explains WHO held the member, stdout was:\n{stdout}"
    );
}

#[test]
fn one_external_caller_dissolves_the_island_whole() {
    let temp = tempfile::tempdir().unwrap();
    write_ring(temp.path());
    fs::write(
        temp.path().join("Caller.kt"),
        "package sample\n\nfun main2() {\n    ringA()\n}\n",
    )
    .unwrap();
    // main2 is itself dead — but reference the ring from live main instead:
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    ringA()\n}\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("ringB"),
        "life propagates through the ring: no member is reported, stdout was:\n{stdout}"
    );
}

#[test]
fn an_xml_token_is_a_root() {
    let temp = tempfile::tempdir().unwrap();
    write_ring(temp.path());
    fs::create_dir_all(temp.path().join("res/layout")).unwrap();
    fs::write(
        temp.path().join("res/layout/a.xml"),
        "<View android:onClick=\"ringA\" />",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("ringB"),
        "a token in XML keeps the island alive, stdout was:\n{stdout}"
    );
}

#[test]
fn a_string_literal_in_live_code_is_a_root() {
    let temp = tempfile::tempdir().unwrap();
    write_ring(temp.path());
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"ringA\")\n}\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("ringB"),
        "reflection lives in string literals: the island stays alive, stdout was:\n{stdout}"
    );
}

#[test]
fn a_string_literal_inside_the_island_does_not_resurrect_it() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Ring.kt"),
        concat!(
            "package sample\n\n",
            "fun ringA() {\n    println(\"ringB calling\")\n    ringB()\n}\n\n",
            "fun ringB() {\n    println(\"ringA done\")\n    ringA()\n}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"app\")\n}\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("ringA") && stdout.contains("ringB"),
        "a self-referential literal is attributed, not a root, stdout was:\n{stdout}"
    );
}

#[test]
fn a_test_reference_marks_the_island_instead_of_reviving_it() {
    let temp = tempfile::tempdir().unwrap();
    write_ring(temp.path());
    let test_dir = temp.path().join("src/test/kotlin");
    fs::create_dir_all(&test_dir).unwrap();
    fs::write(
        test_dir.join("RingTest.kt"),
        "package sample\n\nfun exercise() {\n    ringA()\n}\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("ringA") && stdout.contains("test-only"),
        "a test caller labels the island, deletion stays a human call, stdout was:\n{stdout}"
    );
}

#[test]
fn a_foreign_annotation_saves_the_node_and_roots_its_contents() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Ring.kt"),
        concat!(
            "package sample\n\n",
            "@Keep\n",
            "fun ringA() {\n    ringB()\n}\n\n",
            "fun ringB() {\n    ringA()\n}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"app\")\n}\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("ringB"),
        "the guard that saves a node saves its island, stdout was:\n{stdout}"
    );
}

#[test]
fn a_keepclassmembers_catch_all_does_not_bless_the_corpus() {
    // The Otto idiom `-keepclassmembers class ** { @Subscribe public *; }`
    // keeps MEMBERS, never classes — parsed as a keep-all it turned every
    // declaration of a real corpus into a retention root and blinded the
    // whole deep analysis. Islands must still surface next to it.
    let temp = tempfile::tempdir().unwrap();
    write_ring(temp.path());
    fs::write(
        temp.path().join("proguard-rules.pro"),
        "-keepclassmembers class ** {\n    @com.squareup.otto.Subscribe public *;\n}\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("ringA"),
        "a member-scoped keep rule retains no class, stdout was:\n{stdout}"
    );
}

#[test]
fn a_real_keep_rule_still_roots_its_target() {
    let temp = tempfile::tempdir().unwrap();
    write_ring(temp.path());
    fs::write(
        temp.path().join("proguard-rules.pro"),
        "-keep class sample.ringA\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("ringB"),
        "a targeted -keep is a retention root: the kept name's island lives, stdout was:\n{stdout}"
    );
}

#[test]
fn a_java_override_annotation_saves_the_member() {
    // P1 (île 9) : Java range "@Override" dans modifiers, Kotlin "override" —
    // les deux doivent sortir de la population.
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Base.java"),
        concat!(
            "package sample;\n\n",
            "public class Base implements android.view.ViewTreeObserver.OnPreDrawListener {\n",
            "    @Override\n",
            "    public boolean onPreDraw() {\n",
            "        return helperOnly();\n",
            "    }\n\n",
            "    private boolean helperOnly() {\n",
            "        return true;\n",
            "    }\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(Base())\n}\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("onPreDraw") && !stdout.contains("helperOnly"),
        "un callback @Override et ce qu'il tient ne sont pas une île, stdout was:\n{stdout}"
    );
}

#[test]
fn a_java_method_under_a_supertyped_class_is_out_of_the_population() {
    // P2 (île 9, garde M3) : @Override est optionnel en Java — sous une classe
    // à supertype, un nom peut implémenter une interface hors corpus.
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Director.java"),
        concat!(
            "package sample;\n\n",
            "public class Director implements android.view.ViewTreeObserver.OnPreDrawListener {\n",
            "    public boolean onPreDraw() {\n",
            "        return sizeOnly();\n",
            "    }\n\n",
            "    private boolean sizeOnly() {\n",
            "        return false;\n",
            "    }\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(Director())\n}\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("onPreDraw"),
        "le framework appelle par l'interface, sans @Override visible, stdout was:\n{stdout}"
    );
}

#[test]
fn a_companion_under_a_module_class_is_di_convention_not_an_island() {
    // P7 (île 8) : les companions *ProvideModule de classes @Module ne sont
    // jamais nommés en source — les factories KSP générées vivent dans
    // build/. Le parent @Module enracine ses enfants directs.
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("HomeModule.kt"),
        concat!(
            "package sample\n\n",
            "@dagger.Module\n",
            "class HomeBindingModule {\n",
            "    companion object HomeProvideModule {\n",
            "        @dagger.Provides\n",
            "        fun provideRouter(): SharedRouter = SharedRouter()\n",
            "    }\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("ProfileModule.kt"),
        concat!(
            "package sample\n\n",
            "@dagger.Module\n",
            "class ProfileBindingModule {\n",
            "    companion object ProfileProvideModule {\n",
            "        @dagger.Provides\n",
            "        fun provideProfileRouter(): SharedRouter = SharedRouter()\n",
            "    }\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("SharedRouter.kt"),
        "package sample\n\nclass SharedRouter\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"app\")\n}\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("ProvideModule"),
        "un companion sous @Module est de la convention DI, pas une île, stdout:\n{stdout}"
    );
}

#[test]
fn a_homonym_holder_is_named_by_its_file_rather_than_dropped() {
    // Une île dont des membres partagent un nom se groupe par les arêtes de
    // résolution par nom simple, et s'affichait SANS ligne d'explication :
    // le filtre `source.name != decl.name` retirait le seul détenteur qu'il
    // y avait à nommer. C'est l'île que le lecteur a le plus besoin de
    // comprendre, puisque le groupement vient justement de l'homonymie.
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("A.kt"),
        "package sample\n\nfun helper() {\n    shared()\n}\n\nfun shared() {\n    helper()\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("B.kt"),
        "package sample\n\nfun shared() {\n    helper()\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"app\")\n}\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("dead island"),
        "la paire mutuelle homonyme forme bien une île, stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("shared (B.kt:3)") && stdout.contains("shared (A.kt:7)"),
        "un détenteur homonyme est nommé par son fichier et sa ligne, stdout:\n{stdout}"
    );
    // Chaque membre rapporté porte sa raison.
    let members = stdout.matches("   - ").count();
    let reasons = stdout.matches("kept alive only by").count();
    assert_eq!(
        members, reasons,
        "chaque membre a sa ligne d'explication, stdout:\n{stdout}"
    );
}
