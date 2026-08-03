//! The batch `--delete` path, which once corrupted any file carrying two
//! findings: deletions were applied one at a time, each shifting the byte
//! offsets the next one trusted. The second removal then hit whatever sat at
//! the stale position — while still printing a checkmark. Measured on the
//! published 0.15.1.

use std::fs;
use std::path::Path;
use std::process::Output;

fn run(dir: &Path, extra: &[&str]) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .args(extra)
        .output()
        .unwrap()
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

const LIB_WITH_TWO_DEAD: &str = concat!(
    "package s\n\n",
    "fun deadOne(): Int = 1\n\n",
    "fun liveOne(): Int = 42\n\n",
    "fun deadTwo(): Int = 2\n\n",
    "fun liveTwo(): Int = 7\n",
);

fn write_fixture(dir: &Path) {
    fs::write(dir.join("Lib.kt"), LIB_WITH_TWO_DEAD).unwrap();
    fs::write(
        dir.join("Main.kt"),
        "package s\n\nfun main() {\n    println(liveOne() + liveTwo())\n}\n",
    )
    .unwrap();
}

#[test]
fn two_deletions_in_one_file_each_remove_their_own_lines() {
    let temp = tempfile::tempdir().unwrap();
    write_fixture(temp.path());

    let stdout = stdout_of(&run(temp.path(), &["--delete", "--yes"]));
    let remaining = fs::read_to_string(temp.path().join("Lib.kt")).unwrap();

    assert!(
        stdout.contains("deadOne") && stdout.contains("deadTwo"),
        "both deletions reported, stdout:\n{stdout}"
    );
    assert!(
        !remaining.contains("deadOne") && !remaining.contains("deadTwo"),
        "both dead functions actually removed, file:\n{remaining}"
    );
    assert!(
        remaining.contains("liveOne") && remaining.contains("liveTwo"),
        "no live neighbour was eaten by a shifted offset, file:\n{remaining}"
    );
}

#[test]
fn a_parameter_finding_never_reaches_the_file() {
    // Removing a parameter changes the function's signature, and no call
    // site is rewritten to match: --delete must leave it alone and say so.
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("C.kt"),
        "package s\n\nfun compute(used: Int, neverRead: String): Int {\n    return used * 2\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package s\n\nfun main() {\n    println(compute(1, \"x\"))\n}\n",
    )
    .unwrap();
    let before = fs::read_to_string(temp.path().join("C.kt")).unwrap();

    let stdout = stdout_of(&run(temp.path(), &["--delete", "--yes"]));
    let after = fs::read_to_string(temp.path().join("C.kt")).unwrap();

    assert_eq!(before, after, "the file must not change, stdout:\n{stdout}");
    assert!(
        stdout.contains("left in place"),
        "the refusal is said out loud, stdout:\n{stdout}"
    );
}

#[test]
fn dry_run_promises_exactly_what_the_real_run_delivers() {
    // The preview and the deletion go through the same gates: a dry run
    // that shows a parameter as deletable promises a change the real run
    // refuses to make.
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("C.kt"),
        concat!(
            "package s\n\n",
            "fun compute(used: Int, neverRead: String): Int {\n",
            "    return used * 2\n",
            "}\n\n",
            "fun deadHelper(): Int = 9\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package s\n\nfun main() {\n    println(compute(1, \"x\"))\n}\n",
    )
    .unwrap();

    let dry = stdout_of(&run(temp.path(), &["--delete", "--dry-run", "--yes"]));

    assert!(
        dry.contains("Total: 1 items would be deleted"),
        "only the function is promised, stdout:\n{dry}"
    );
    assert!(
        dry.contains("left in place"),
        "the parameter gate shows in the preview too, stdout:\n{dry}"
    );

    let real = stdout_of(&run(temp.path(), &["--delete", "--yes"]));
    let after = fs::read_to_string(temp.path().join("C.kt")).unwrap();

    assert!(
        real.contains("Deleted function 'deadHelper'") && !after.contains("deadHelper"),
        "the promised deletion happened, stdout:\n{real}"
    );
    assert!(
        after.contains("neverRead"),
        "the refused deletion did not, file:\n{after}"
    );
}
