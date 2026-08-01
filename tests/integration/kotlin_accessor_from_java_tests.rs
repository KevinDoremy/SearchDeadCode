//! Java lit une propriété Kotlin par son accesseur JVM généré :
//! `Holder.Companion.getMAX_ITEMS()` pour `val MAX_ITEMS`, `getLabel()`
//! pour `val label`. Le nom appelé ne correspond textuellement à aucune
//! déclaration — sans repli sur la propriété, tout ce que Java lit d'un
//! fichier Kotlin passait pour mort.

use std::fs;
use std::path::Path;
use std::process::Output;

fn run(dir: &Path) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .output()
        .unwrap()
}

#[test]
fn a_kotlin_property_read_from_java_via_its_accessor_is_alive() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    Gate.check()\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Holder.kt"),
        concat!(
            "package sample\n\n",
            "class Holder {\n",
            "    val label: String = \"x\"\n\n",
            "    companion object {\n",
            "        val MAX_ITEMS: Int = 4\n",
            "    }\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Gate.java"),
        concat!(
            "package sample;\n\n",
            "public class Gate {\n",
            "    public static void check() {\n",
            "        int max = Holder.Companion.getMAX_ITEMS();\n",
            "        String name = new Holder().getLabel();\n",
            "        System.out.println(max + name);\n",
            "    }\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = String::from_utf8_lossy(&run(temp.path()).stdout).to_string();
    assert!(
        !stdout.contains("MAX_ITEMS"),
        "une const companion lue depuis Java est vivante, stdout:\n{stdout}"
    );
    assert!(
        !stdout.contains("'label'"),
        "une propriété lue depuis Java est vivante, stdout:\n{stdout}"
    );
}

#[test]
fn a_java_homonym_does_not_mask_the_kotlin_property_behind_the_accessor() {
    // P4 (île 10) : getScreenContentWidth() résolvait vers un getter Java
    // homonyme sans rapport (find_by_name non vide → return) et n'atteignait
    // jamais la propriété Kotlin — le pont accesseur doit être une union.
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    Splash.dump()\n    println(Widget())\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Prefs.kt"),
        "package sample\n\nclass Prefs {\n    var screenContentWidth: Int = 0\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Widget.java"),
        concat!(
            "package sample;\n\n",
            "public class Widget {\n",
            "    public int getScreenContentWidth() {\n",
            "        return 42;\n",
            "    }\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Splash.java"),
        concat!(
            "package sample;\n\n",
            "public class Splash {\n",
            "    public static void dump() {\n",
            "        System.out.println(new Prefs().getScreenContentWidth());\n",
            "    }\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = String::from_utf8_lossy(&run(temp.path()).stdout).to_string();
    assert!(
        !stdout.contains("screenContentWidth"),
        "la propriété lue via son accesseur reste vivante malgré l'homonyme Java, stdout:\n{stdout}"
    );
}

#[test]
fn a_java_getter_read_from_kotlin_as_a_property_is_alive() {
    // Le miroir du pont : Kotlin voit `getX()`/`setX()` d'une classe Java
    // comme la propriété synthétique `x`, donc `button.interactionCount`
    // EST un appel de `getInteractionCount()`. Deux des trois faux positifs
    // de l'audit --deep du monorepo venaient de là.
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Button.java"),
        concat!(
            "package sample;\n\n",
            "public class Button {\n",
            "    private int count = 0;\n\n",
            "    public int getInteractionCount() {\n",
            "        return count;\n",
            "    }\n\n",
            "    public void setInteractionCount(int value) {\n",
            "        count = value;\n",
            "    }\n\n",
            "    public boolean isReady() {\n",
            "        return true;\n",
            "    }\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        concat!(
            "package sample\n\n",
            "fun main() {\n",
            "    val button = Button()\n",
            "    val old = button.interactionCount\n",
            "    button.interactionCount = old + 1\n",
            "    println(button.isReady)\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = String::from_utf8_lossy(&run(temp.path()).stdout).to_string();
    for accessor in ["getInteractionCount", "setInteractionCount", "isReady"] {
        assert!(
            !stdout.contains(accessor),
            "{accessor} est atteint par la propriété synthétique Kotlin, stdout:\n{stdout}"
        );
    }
}

#[test]
fn a_local_val_does_not_resurrect_a_java_getter_of_that_name() {
    // Le pont propriété-synthétique doit tenir à la SYNTAXE : `button.label`
    // est un appel de `getLabel()`, un `val label` local ne l'est pas.
    // Câblé côté résolution, il ressuscitait tout getter Java homonyme du
    // corpus, sans jamais regarder le type du receveur.
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Widget.java"),
        concat!(
            "package sample;\n\n",
            "public class Widget {\n",
            "    public String getLabel() {\n",
            "        return \"x\";\n",
            "    }\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        concat!(
            "package sample\n\n",
            "fun main() {\n",
            "    val label = \"sans rapport avec Widget\"\n",
            "    println(label.length)\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = String::from_utf8_lossy(&run(temp.path()).stdout).to_string();
    assert!(
        stdout.contains("Widget"),
        "un getter que personne n'appelle reste mort malgré un homonyme local, stdout:\n{stdout}"
    );
}
