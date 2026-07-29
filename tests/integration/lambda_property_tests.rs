//! Une propriété de type fonction est LUE quand on l'invoque :
//! `holder.onDone(true)` lit la lambda avant de l'appeler. La référence
//! est classée Call (l'appel), pas Read — la propriété passait donc pour
//! écrite-jamais-lue alors qu'elle porte tout le flux de callbacks.

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
fn an_invoked_lambda_property_is_not_write_only() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        concat!(
            "package sample\n\n",
            "fun main() {\n",
            "    Runner().start {\n",
            "        onDone = { println(it) }\n",
            "        onStart = { println(\"go\") }\n",
            "    }\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Runner.kt"),
        concat!(
            "package sample\n\n",
            "open class Runner {\n\n",
            "    fun start(builder: Params.() -> Unit) {\n",
            "        val params = Params(builder)\n",
            "        params.onDone(true)\n",
            "        params.onStart()\n",
            "    }\n\n",
            "    class Params(builder: Params.() -> Unit) {\n",
            "        internal lateinit var onDone: (ok: Boolean) -> Unit\n",
            "        internal lateinit var onStart: () -> Unit\n\n",
            "        init {\n",
            "            builder()\n",
            "        }\n",
            "    }\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = String::from_utf8_lossy(&run(temp.path()).stdout).to_string();
    assert!(
        !stdout.contains("onDone"),
        "invoquer une propriété-lambda la lit, stdout:\n{stdout}"
    );
}
