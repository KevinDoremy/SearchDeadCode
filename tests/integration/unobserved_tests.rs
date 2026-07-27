//! Integration tests for --unobserved: a LiveData/StateFlow/SharedFlow
//! exposed by a ViewModel that no screen ever collects or observes —
//! the whole upstream computation runs for nobody.

use std::fs;
use std::path::Path;
use std::process::Output;

fn run(dir: &Path) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .arg("--unobserved")
        .output()
        .unwrap()
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn a_stateflow_nobody_collects_is_flagged() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("CounterModel.kt"),
        concat!(
            "package sample\n\n",
            "class CounterModel {\n",
            "    val ticks: StateFlow<Int> = MutableStateFlow(0)\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        concat!(
            "package sample\n\n",
            "fun main() {\n",
            "    val model = CounterModel()\n",
            "    println(model)\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("ticks"),
        "the never-collected StateFlow is flagged, stdout was:\n{stdout}"
    );
}

#[test]
fn a_collected_stateflow_is_not_flagged() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("CounterModel.kt"),
        concat!(
            "package sample\n\n",
            "class CounterModel {\n",
            "    val ticks: StateFlow<Int> = MutableStateFlow(0)\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Screen.kt"),
        concat!(
            "package sample\n\n",
            "suspend fun render(model: CounterModel) {\n",
            "    model.ticks.collect { println(it) }\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("ticks"),
        "a collected flow is alive, stdout was:\n{stdout}"
    );
}

#[test]
fn an_observed_livedata_is_not_flagged() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("UserModel.kt"),
        concat!(
            "package sample\n\n",
            "class UserModel {\n",
            "    val userName: LiveData<String> = MutableLiveData()\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Screen.kt"),
        concat!(
            "package sample\n\n",
            "fun bind(model: UserModel) {\n",
            "    model.userName.observe(owner) { title = it }\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("userName"),
        "an observed LiveData is alive, stdout was:\n{stdout}"
    );
}

#[test]
fn a_private_backing_flow_is_not_flagged() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("StateModel.kt"),
        concat!(
            "package sample\n\n",
            "class StateModel {\n",
            "    private val _uiState = MutableStateFlow(0)\n",
            "    val uiState: StateFlow<Int> = _uiState\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Screen.kt"),
        concat!(
            "package sample\n\n",
            "suspend fun render(model: StateModel) {\n",
            "    model.uiState.collect { println(it) }\n",
            "}\n",
        ),
    )
    .unwrap();

    let output = run(temp.path());
    let stdout = stdout_of(&output);
    assert!(
        output.status.success(),
        "the classic backing-field pattern is clean, output was:\n{output:?}"
    );
    assert!(
        stdout.to_lowercase().contains("no unobserved streams"),
        "the private backing field is not noise, stdout was:\n{stdout}"
    );
}

#[test]
fn a_compose_collect_as_state_counts_as_observed() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("ScreenModel.kt"),
        concat!(
            "package sample\n\n",
            "class ScreenModel {\n",
            "    val uiState: StateFlow<Int> = MutableStateFlow(0)\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Screen.kt"),
        concat!(
            "package sample\n\n",
            "fun Screen(model: ScreenModel) {\n",
            "    val state by model.uiState.collectAsStateWithLifecycle()\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("uiState"),
        "compose-style collection is observation, stdout was:\n{stdout}"
    );
}

#[test]
fn an_on_each_launch_in_chain_counts_as_observed() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("EventModel.kt"),
        concat!(
            "package sample\n\n",
            "class EventModel {\n",
            "    val events: SharedFlow<String> = MutableSharedFlow()\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Wiring.kt"),
        concat!(
            "package sample\n\n",
            "fun wire(model: EventModel, scope: CoroutineScope) {\n",
            "    model.events.onEach { handle(it) }.launchIn(scope)\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("events"),
        "an operator chain ending in launchIn is observation, stdout was:\n{stdout}"
    );
}

#[test]
fn an_override_stream_property_is_still_checked() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Impl.kt"),
        concat!(
            "package sample\n\n",
            "class Impl : Contract {\n",
            "    override val results: Flow<Int> = flowOf(1)\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(Impl())\n}\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("results"),
        "modifier prefixes don't hide a dead stream, stdout was:\n{stdout}"
    );
}

#[test]
fn no_streams_is_a_clean_answer() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();

    let output = run(temp.path());
    let stdout = stdout_of(&output);
    assert!(
        output.status.success(),
        "no streams is fine, output was:\n{output:?}"
    );
    assert!(
        stdout.to_lowercase().contains("no unobserved streams"),
        "the verdict is explicit, stdout was:\n{stdout}"
    );
}
