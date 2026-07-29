//! Cas réel : une custom view dont onFinishInflate (@Override, blessé
//! par la passe « additional ») construit un contrôleur, dont le ctor
//! construit un Handler nested ; le handleMessage (@Override) de ce
//! dernier n'est jamais blessé — la passe ne tourne qu'une fois, donc
//! toute chaîne alternant override → appel → override casse au deuxième
//! override et ses callees sortent en "only referenced from dead code".

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
fn a_second_override_link_in_the_chain_keeps_its_callees_alive() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        concat!(
            "package sample\n\n",
            "fun main() {\n",
            "    println(Screen())\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Screen.kt"),
        concat!(
            "package sample\n\n",
            "abstract class ScreenBase {\n",
            "    open fun onReady() {}\n",
            "}\n\n",
            "class Screen : ScreenBase() {\n",
            "    override fun onReady() {\n",
            "        println(Widget(3))\n",
            "    }\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("BaseHandler.java"),
        concat!(
            "package sample;\n\n",
            "public class BaseHandler {\n",
            "    public void handleMessage() {}\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Widget.java"),
        concat!(
            "package sample;\n\n",
            "public class Widget {\n",
            "    private final InnerHandler handler;\n\n",
            "    public Widget(int x) {\n",
            "        handler = new InnerHandler();\n",
            "    }\n\n",
            "    static class InnerHandler extends BaseHandler {\n",
            "        @Override\n",
            "        public void handleMessage() {\n",
            "            refreshNow();\n",
            "        }\n\n",
            "        private void refreshNow() {\n",
            "            System.out.println(\"tick\");\n",
            "        }\n",
            "    }\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = String::from_utf8_lossy(&run(temp.path()).stdout).to_string();
    assert!(
        !stdout.contains("refreshNow"),
        "le callee d'un deuxième override de la chaîne est vivant, stdout:\n{stdout}"
    );
    // BaseHandler.handleMessage "never used" est un vrai finding dans
    // cette fixture (personne ne l'appelle) — on ne vérifie que la
    // chaîne Widget : l'override d'InnerHandler et son callee.
    assert!(
        !stdout.contains("Widget.java"),
        "un override d'une classe atteinte tardivement est vivant, stdout:\n{stdout}"
    );
}

#[test]
fn a_reachable_node_inserted_without_dfs_still_walks_its_edges() {
    // Cas réel : une classe vivante par ses seuls @Subscribe, dont le
    // ctor Java (`new Inner(this)`, argument + ctor explicite) construit
    // un handler nested ; le handleMessage (@Override) finissait
    // "reachable" par insertion directe sans DFS — ses callees privés
    // sortaient en "only referenced from dead code".
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        concat!(
            "package sample\n\n",
            "fun main() {\n",
            "    println(AudioWidget(3))\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("PingEvent.kt"),
        "package sample\n\nclass PingEvent\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("BaseHandler.java"),
        concat!(
            "package sample;\n\n",
            "public class BaseHandler {\n",
            "    public void handleMessage() {}\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("AudioWidget.java"),
        concat!(
            "package sample;\n\n",
            "public class AudioWidget {\n",
            "    private final InnerHandler handler;\n\n",
            "    public AudioWidget(int x) {\n",
            "        handler = new InnerHandler(this);\n",
            "    }\n\n",
            "    @Subscribe\n",
            "    public void onBusEvent(PingEvent event) {\n",
            "        System.out.println(event);\n",
            "    }\n\n",
            "    private void refreshNow() {\n",
            "        System.out.println(\"tick\");\n",
            "    }\n\n",
            "    static class InnerHandler extends BaseHandler {\n",
            "        private final AudioWidget widget;\n\n",
            "        InnerHandler(AudioWidget w) {\n",
            "            widget = w;\n",
            "        }\n\n",
            "        @Override\n",
            "        public void handleMessage() {\n",
            "            widget.refreshNow();\n",
            "        }\n",
            "    }\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = String::from_utf8_lossy(&run(temp.path()).stdout).to_string();
    assert!(
        !stdout.contains("refreshNow"),
        "le callee d'un override raccroché sans DFS est vivant, stdout:\n{stdout}"
    );
}
