//! Cas réel : les méthodes @JavascriptInterface sont invoquées par
//! réflexion depuis le JS du WebView — aucun appelant statique. Sans
//! blessing, toute la chaîne (bridge → use case → helpers privés) sort
//! en "only referenced from dead code" alors qu'elle tourne en prod.

use std::fs;
use std::path::Path;
use std::process::Output;

fn run(dir: &Path) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .arg("--deep")
        .output()
        .unwrap()
}

#[test]
fn a_javascript_interface_method_and_its_callees_are_alive() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        concat!(
            "package sample\n\n",
            "fun main() {\n",
            "    val bridge = JsBridge()\n",
            "    println(bridge)\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("JsBridge.kt"),
        concat!(
            "package sample\n\n",
            "import android.webkit.JavascriptInterface\n\n",
            "class JsBridge {\n",
            "    @JavascriptInterface\n",
            "    fun openFullscreenImage(json: String) {\n",
            "        handleOpenImage(json)\n",
            "    }\n\n",
            "    private fun handleOpenImage(json: String) {\n",
            "        println(json)\n",
            "    }\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = String::from_utf8_lossy(&run(temp.path()).stdout).to_string();
    assert!(
        !stdout.contains("openFullscreenImage"),
        "une méthode @JavascriptInterface est appelée par le JS du WebView, stdout:\n{stdout}"
    );
    assert!(
        !stdout.contains("handleOpenImage"),
        "le callee d'une méthode @JavascriptInterface est vivant, stdout:\n{stdout}"
    );
}
