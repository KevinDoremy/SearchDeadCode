//! Integration tests for Java parity: write-only prefs and write-only
//! DAO detection only scanned Kotlin files, while a typical mixed repo
//! is half Java — half the codebase escaped the advanced detectors.

use std::fs;
use std::path::Path;
use std::process::Output;

fn run(dir: &Path) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .output()
        .unwrap()
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn write_main(dir: &Path) {
    fs::write(
        dir.join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();
}

#[test]
fn a_java_write_only_pref_is_reported() {
    let temp = tempfile::tempdir().unwrap();
    write_main(temp.path());
    fs::write(
        temp.path().join("JavaPrefs.java"),
        concat!(
            "package sample;\n\n",
            "public class JavaPrefs {\n",
            "    void save(Editor editor) {\n",
            "        editor.putString(\"java_dead_pref\", \"x\");\n",
            "        editor.putString(\"java_live_pref\", \"y\");\n",
            "    }\n\n",
            "    String load(Prefs prefs) {\n",
            "        return prefs.getString(\"java_live_pref\", null);\n",
            "    }\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("java_dead_pref"),
        "a Java key written but never read is a finding, stdout was:\n{stdout}"
    );
    assert!(
        !stdout.contains("\"java_live_pref\" is written but never read")
            && !stdout.contains("java_live_pref\" is written"),
        "a read key is not write-only, stdout was:\n{stdout}"
    );
}

#[test]
fn a_java_write_only_dao_is_reported() {
    let temp = tempfile::tempdir().unwrap();
    write_main(temp.path());
    fs::write(
        temp.path().join("JavaLogDao.java"),
        concat!(
            "package sample;\n\n",
            "@Dao\n",
            "public interface JavaLogDao {\n",
            "    @Insert\n",
            "    void insert(String entry);\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("JavaLogDao"),
        "a Java DAO with @Insert and no @Query is a finding, stdout was:\n{stdout}"
    );
}

#[test]
fn a_java_dao_with_queries_stays_silent() {
    let temp = tempfile::tempdir().unwrap();
    write_main(temp.path());
    fs::write(
        temp.path().join("HealthyDao.java"),
        concat!(
            "package sample;\n\n",
            "@Dao\n",
            "public interface HealthyDao {\n",
            "    @Insert\n",
            "    void insert(String entry);\n\n",
            "    @Query(\"SELECT * FROM log\")\n",
            "    java.util.List<String> all();\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("HealthyDao' has @Insert"),
        "reads exist, nothing write-only, stdout was:\n{stdout}"
    );
}

#[test]
fn a_java_lifecycle_method_without_override_is_retained() {
    // Java's @Override is optional — a framework callback without it is
    // still called by the framework, never by user code
    let temp = tempfile::tempdir().unwrap();
    write_main(temp.path());
    fs::write(
        temp.path().join("CrashActivity.java"),
        concat!(
            "package sample;\n\n",
            "public class CrashActivity extends Activity {\n",
            "    public void onCreate(Bundle savedInstanceState) {\n",
            "        super.onCreate(savedInstanceState);\n",
            "    }\n",
            "}\n",
        ),
    )
    .unwrap();

    let out = std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(temp.path())
        .arg("--deep")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        !stdout.contains("'onCreate'"),
        "the framework calls it, not user code, stdout was:\n{stdout}"
    );
}

#[test]
fn a_lifecycle_name_in_a_plain_class_is_not_blessed() {
    // the retention keys on the parent extending something — a lonely
    // onCreate in a base-less class is just a dead method with a
    // familiar name
    let temp = tempfile::tempdir().unwrap();
    write_main(temp.path());
    fs::write(
        temp.path().join("Impostor.java"),
        concat!(
            "package sample;\n\n",
            "public class Impostor {\n",
            "    public void onCreate(Object ignored) {\n",
            "    }\n",
            "}\n",
        ),
    )
    .unwrap();
    // Impostor itself is alive (constructed from main), so its members
    // are judged individually rather than folded into the class
    fs::write(
        temp.path().join("UseImpostor.kt"),
        "package sample\n\nfun main2() {\n    Impostor()\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    main2()\n}\n",
    )
    .unwrap();

    let out = std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(temp.path())
        .arg("--deep")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("onCreate"),
        "a lifecycle NAME alone earns nothing, stdout was:\n{stdout}"
    );
}

#[test]
fn an_ordinary_dead_method_in_an_activity_is_still_reported() {
    let temp = tempfile::tempdir().unwrap();
    write_main(temp.path());
    fs::write(
        temp.path().join("HomeActivity.java"),
        concat!(
            "package sample;\n\n",
            "public class HomeActivity extends Activity {\n",
            "    public void onCreate(Bundle savedInstanceState) {\n",
            "        super.onCreate(savedInstanceState);\n",
            "    }\n",
            "    public void formatLegacyBanner() {\n",
            "    }\n",
            "}\n",
        ),
    )
    .unwrap();

    let out = std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(temp.path())
        .arg("--deep")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("formatLegacyBanner"),
        "the lifecycle blessing does not cover the neighbors, stdout was:\n{stdout}"
    );
}

#[test]
fn a_java_class_constructed_from_kotlin_keeps_its_ctor_callees_alive() {
    // Cas réel : une classe Java construite depuis un fichier Kotlin
    // d'un autre package via un import. La résolution par import liait
    // l'appel à la classe seulement — le constructeur restait sans
    // arête entrante et ses callees sortaient en "only referenced from
    // dead code". Le même appel depuis le même package passait par le
    // fallback nom-simple, qui lie aussi le constructeur (un ctor Java
    // porte le nom de sa classe).
    let temp = tempfile::tempdir().unwrap();
    fs::create_dir(temp.path().join("cursor")).unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        concat!(
            "package sample.main\n\n",
            "import sample.cursor.CursorHelper\n\n",
            "fun main() {\n",
            "    val helper = CursorHelper(3)\n",
            "    println(helper)\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("cursor/CursorHelper.java"),
        concat!(
            "package sample.cursor;\n\n",
            "public class CursorHelper {\n",
            "    public CursorHelper(int count) {\n",
            "        initFromConstructor(count);\n",
            "    }\n\n",
            "    private void initFromConstructor(int count) {\n",
            "        System.out.println(count);\n",
            "    }\n",
            "}\n",
        ),
    )
    .unwrap();

    let out = std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(temp.path())
        .arg("--deep")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        !stdout.contains("initFromConstructor"),
        "un callee de constructeur atteint depuis Kotlin via import est vivant, stdout:\n{stdout}"
    );
    assert!(
        !stdout.contains("constructor 'CursorHelper' is never used"),
        "le constructeur appelé depuis Kotlin via import est vivant, stdout:\n{stdout}"
    );
}

#[test]
fn a_private_no_arg_utility_constructor_is_an_idiom_not_dead_code() {
    // `private TimeUtils() {}` dans une classe utilitaire finale : le but
    // du constructeur EST de ne jamais être appelé (anti-instanciation).
    // Le flagger « never used » est du bruit — on ne peut pas le
    // supprimer sans réintroduire le ctor public par défaut.
    // Forme cross-package : le receveur `Clock.formatNow()` résolu par
    // import ne lie pas le constructeur homonyme — c'est là que l'idiome
    // sortait « never used ».
    let temp = tempfile::tempdir().unwrap();
    fs::create_dir(temp.path().join("util")).unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        concat!(
            "package sample\n\n",
            "import sample.util.Clock\n\n",
            "fun main() {\n",
            "    println(Clock.formatNow())\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("util/Clock.java"),
        concat!(
            "package sample.util;\n\n",
            "public final class Clock {\n\n",
            "    private Clock() {\n",
            "    }\n\n",
            "    public static String formatNow() {\n",
            "        return \"now\";\n",
            "    }\n",
            "}\n",
        ),
    )
    .unwrap();

    let out = std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(temp.path())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        !stdout.contains("'Clock' is never used"),
        "un ctor privé no-arg est un idiome anti-instanciation, stdout:\n{stdout}"
    );
}

#[test]
fn a_manifest_declared_class_keeps_its_constructor_alive() {
    // Le système Android instancie lui-même ce que le manifest déclare
    // (application, activity, backupAgent…) : le constructeur n'a aucun
    // appelant dans le code, ce qui est normal et non actionnable.
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("AndroidManifest.xml"),
        concat!(
            "<manifest package=\"sample\">\n",
            "    <application android:name=\".App\" android:backupAgent=\".BackupHelper\" />\n",
            "</manifest>\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("BackupHelper.java"),
        concat!(
            "package sample;\n\n",
            "public class BackupHelper extends BackupAgentHelper {\n\n",
            "    public BackupHelper() {\n",
            "    }\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("App.java"),
        "package sample;\n\npublic class App extends Application {\n}\n",
    )
    .unwrap();

    let out = std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(temp.path())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        !stdout.contains("BackupHelper"),
        "une classe déclarée au manifest est instanciée par le système, stdout:\n{stdout}"
    );
}
