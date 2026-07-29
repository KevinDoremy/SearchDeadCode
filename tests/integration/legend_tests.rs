//! Legend-drift guard: the confidence symbols the binary actually
//! prints must be the ones the docs teach. The code shows one set and
//! the docs another — a reader learns symbols that never appear.

use std::fs;
use std::path::Path;
use std::process::Command;

/// The real legend, harvested from a live run with findings.
fn real_legend_line() -> String {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Ghost.kt"),
        "package sample\n\nclass Ghost {\n    fun haunt() {}\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(temp.path())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let legend_at = stdout
        .find("Confidence Legend")
        .expect("the report prints a legend");
    stdout[legend_at..]
        .lines()
        .take(3)
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn the_readme_teaches_the_symbols_the_binary_prints() {
    let legend = real_legend_line();
    let readme =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("README.md")).unwrap();

    for (symbol, name) in [
        ("✓", "Confirmed"),
        ("!", "High"),
        ("?", "Medium"),
        ("~", "Low"),
    ] {
        assert!(
            legend.contains(symbol),
            "sanity: the binary prints {symbol} for {name}, legend was:\n{legend}"
        );
        assert!(
            readme.contains(&format!("{symbol} {name}"))
                || readme.contains(&format!("{symbol}  {name}")),
            "README must teach '{symbol} {name}' — the symbols the binary prints"
        );
    }
    for stale in ["● Confirmed", "◉ High", "○ Medium", "◌ Low"] {
        assert!(
            !readme.contains(stale),
            "README still teaches the old symbol set: {stale}"
        );
    }
}

#[test]
fn the_detector_docs_use_the_real_symbols_too() {
    for doc in ["docs/detectors.md", "docs/hybrid-analysis.md"] {
        let content = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(doc)).unwrap();
        for stale in ["● green", "◉ bright green", "◌ red"] {
            assert!(
                !content.contains(stale),
                "{doc} still documents the old symbol set: {stale}"
            );
        }
    }
}
