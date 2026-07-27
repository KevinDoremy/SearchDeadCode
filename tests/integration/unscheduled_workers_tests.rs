//! Integration tests for --unscheduled-workers: a Worker/JobService
//! nobody ever enqueues is background code that will never run — and
//! framework retention (name/inheritance) hides it from the report.

use std::fs;
use std::path::Path;
use std::process::Output;

fn run(dir: &Path) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .arg("--unscheduled-workers")
        .output()
        .unwrap()
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn a_worker_nobody_enqueues_is_flagged() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("OrphanSync.kt"),
        concat!(
            "package sample\n\n",
            "class OrphanSync(ctx: Context, params: WorkerParameters) : CoroutineWorker(ctx, params) {\n",
            "    override suspend fun doWork(): Result = Result.success()\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("OrphanSync"),
        "background code that will never run, stdout was:\n{stdout}"
    );
}

#[test]
fn an_enqueued_worker_is_not_flagged() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("UploadTask.kt"),
        concat!(
            "package sample\n\n",
            "class UploadTask(ctx: Context, params: WorkerParameters) : CoroutineWorker(ctx, params) {\n",
            "    override suspend fun doWork(): Result = Result.success()\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Scheduler.kt"),
        concat!(
            "package sample\n\n",
            "fun schedule(manager: WorkManager) {\n",
            "    val request = OneTimeWorkRequestBuilder<UploadTask>().build()\n",
            "    manager.enqueue(request)\n",
            "}\n\n",
            "fun main() {\n    println(\"alive\")\n}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("UploadTask"),
        "the WorkRequest reference schedules it, stdout was:\n{stdout}"
    );
}

#[test]
fn a_plain_class_is_out_of_scope() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Loner.kt"),
        "package sample\n\nclass Loner {\n    fun sit() {}\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("Loner"),
        "only workers and job services are in scope, stdout was:\n{stdout}"
    );
}

#[test]
fn no_workers_is_a_clean_answer() {
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
        stdout.to_lowercase().contains("no unscheduled workers"),
        "the verdict is explicit, stdout was:\n{stdout}"
    );
}
