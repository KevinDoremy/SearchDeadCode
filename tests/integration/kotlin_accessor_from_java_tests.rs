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
