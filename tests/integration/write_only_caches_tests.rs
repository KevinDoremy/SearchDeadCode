//! Integration tests for --write-only-caches: a cache key written but
//! never read back means the whole compute-and-store pipeline runs for
//! nothing — the cache flavor of write-only preferences.

use std::fs;
use std::path::Path;
use std::process::Output;

fn run(dir: &Path) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .arg("--write-only-caches")
        .output()
        .unwrap()
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn a_key_written_but_never_read_is_flagged() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Warmup.kt"),
        concat!(
            "package sample\n\n",
            "fun warm(cache: LruCache<String, Int>) {\n",
            "    cache.put(\"user_score\", 42)\n",
            "}\n\n",
            "fun main() {\n    println(\"alive\")\n}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("user_score"),
        "the never-read key is flagged, stdout was:\n{stdout}"
    );
}

#[test]
fn a_key_read_back_is_not_flagged() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Roundtrip.kt"),
        concat!(
            "package sample\n\n",
            "fun warm(cache: LruCache<String, Int>) {\n",
            "    cache.put(\"user_score\", 42)\n",
            "}\n\n",
            "fun read(cache: LruCache<String, Int>): Int? {\n",
            "    return cache.get(\"user_score\")\n",
            "}\n\n",
            "fun main() {\n    println(\"alive\")\n}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("user_score"),
        "a round-tripped key is healthy, stdout was:\n{stdout}"
    );
}

#[test]
fn non_cache_receivers_are_out_of_scope() {
    // a map named anything else is not judged — the heuristic keys on
    // cache-flavored receiver names to stay honest about its reach
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Registry.kt"),
        concat!(
            "package sample\n\n",
            "fun record(registry: MutableMap<String, Int>) {\n",
            "    registry.put(\"session_count\", 1)\n",
            "}\n\n",
            "fun main() {\n    println(\"alive\")\n}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("session_count"),
        "only cache-named receivers are judged, stdout was:\n{stdout}"
    );
}

#[test]
fn no_cache_writes_is_a_clean_answer() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();

    let output = run(temp.path());
    let stdout = stdout_of(&output);
    assert!(output.status.success());
    assert!(
        stdout.to_lowercase().contains("no cache writes"),
        "no writes at all is its own explicit verdict, stdout was:\n{stdout}"
    );
}
