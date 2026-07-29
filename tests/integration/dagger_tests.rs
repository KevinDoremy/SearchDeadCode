//! Integration tests for DI binding resolution (Dagger-style @Provides/@Binds).
//!
//! A provider method is only a root when its produced type is actually
//! consumed (injected field, provider parameter). An orphan module whose
//! bindings nobody consumes is dead code like any other.

use std::fs;
use std::path::Path;
use std::process::Output;

/// Consumed chain: Main -> AppComponent.inject(Dashboard), Dashboard has an
/// @Inject field of type UsageTracker, provided by TrackerModule.
/// Orphan chain: OrphanModule provides OrphanGadget, which nobody consumes.
fn write_sample_project(dir: &Path) {
    fs::write(
        dir.join("Main.kt"),
        "package sample\n\nfun main() {\n    val component = AppComponent()\n    component.inject(Dashboard())\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("AppComponent.kt"),
        "package sample\n\nclass AppComponent {\n    fun inject(target: Dashboard) {}\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("Dashboard.kt"),
        "package sample\n\nclass Dashboard {\n    @Inject lateinit var tracker: UsageTracker\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("UsageTracker.kt"),
        "package sample\n\ninterface UsageTracker {\n    fun track(event: String)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("TrackerModule.kt"),
        "package sample\n\n@Module\nclass TrackerModule {\n    @Provides\n    fun provideTracker(): UsageTracker = DefaultUsageTracker()\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("DefaultUsageTracker.kt"),
        "package sample\n\nclass DefaultUsageTracker : UsageTracker {\n    override fun track(event: String) {}\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("OrphanModule.kt"),
        "package sample\n\n@Module\nclass OrphanModule {\n    @Provides\n    fun provideOrphan(): OrphanGadget = OrphanGadget()\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("OrphanGadget.kt"),
        "package sample\n\nclass OrphanGadget {\n    fun serve() {}\n}\n",
    )
    .unwrap();
}

fn run(dir: &Path) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .output()
        .unwrap()
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn orphan_provided_class_is_flagged_dead() {
    let temp = tempfile::tempdir().unwrap();
    write_sample_project(temp.path());

    let output = run(temp.path());

    let stdout = stdout_of(&output);
    assert!(
        stdout.contains("'OrphanGadget'"),
        "a provided class nobody consumes is dead, stdout was:\n{stdout}"
    );
    assert!(
        stdout.contains("'provideOrphan'"),
        "the provider of an unconsumed type is dead too, stdout was:\n{stdout}"
    );
}

#[test]
fn consumed_binding_chain_stays_alive() {
    let temp = tempfile::tempdir().unwrap();
    write_sample_project(temp.path());

    let output = run(temp.path());

    let stdout = stdout_of(&output);
    for alive in ["DefaultUsageTracker", "provideTracker", "UsageTracker"] {
        assert!(
            !stdout.contains(&format!("'{alive}'")),
            "{alive} is consumed through the binding chain and must stay alive, stdout was:\n{stdout}"
        );
    }
}

#[test]
fn provider_parameter_counts_as_consumption() {
    let temp = tempfile::tempdir().unwrap();
    write_sample_project(temp.path());
    fs::write(
        temp.path().join("EngineModule.kt"),
        "package sample\n\n@Module\nclass EngineModule {\n    @Provides\n    fun provideEngine(): Engine = Engine()\n\n    @Provides\n    fun provideCar(engine: Engine): Car = Car(engine)\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Engine.kt"),
        "package sample\n\nclass Engine {\n    fun rev() {}\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Car.kt"),
        "package sample\n\nclass Car(private val engine: Engine) {\n    fun drive() {}\n}\n",
    )
    .unwrap();
    // Car is consumed by an injected field
    fs::write(
        temp.path().join("Garage.kt"),
        "package sample\n\nclass Garage {\n    @Inject lateinit var car: Car\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    val component = AppComponent()\n    component.inject(Dashboard())\n    Garage()\n}\n",
    )
    .unwrap();

    let output = run(temp.path());

    let stdout = stdout_of(&output);
    for alive in ["Engine", "provideEngine", "Car", "provideCar"] {
        assert!(
            !stdout.contains(&format!("'{alive}'")),
            "{alive} is consumed through provider parameters, stdout was:\n{stdout}"
        );
    }
}

#[test]
fn a_provider_of_an_external_type_gets_the_benefit_of_the_doubt() {
    // Cas réel : `@Provides fun providesIoDispatcher(): CoroutineDispatcher`
    // — le type produit vient d'une lib (kotlinx), il n'a AUCUNE
    // déclaration dans le graphe, donc « produced type consumed? » ne
    // trouvait jamais rien et condamnait tous les providers de types
    // externes (dispatchers, SharedPreferences, players…). Type
    // introuvable dans le projet = indécidable = racine.
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"up\")\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("ThreadingModule.kt"),
        concat!(
            "package sample\n\n",
            "import kotlinx.coroutines.CoroutineDispatcher\n",
            "import kotlinx.coroutines.Dispatchers\n\n",
            "@Module\n",
            "class ThreadingModule {\n",
            "    @Provides\n",
            "    fun providesIoDispatcher(): CoroutineDispatcher = Dispatchers.IO\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("providesIoDispatcher"),
        "un provider d'un type externe au projet n'est pas condamnable, stdout:\n{stdout}"
    );
}

#[test]
fn a_consumer_that_also_implements_the_bound_interface_still_counts() {
    // Cas réel : `class Delegate(inner: Formatter) : Formatter by inner`
    // — le consommateur reçoit l'interface en ctor ET l'implémente par
    // délégation. Le skip « référenceur-implémenteur » jetait sa
    // consommation légitime et le @Binds sortait « never used ».
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(Delegate(FormatterImpl()))\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Formatter.kt"),
        concat!(
            "package sample\n\n",
            "interface Formatter {\n",
            "    fun format(): String\n",
            "}\n\n",
            "class FormatterImpl : Formatter {\n",
            "    override fun format(): String = \"x\"\n",
            "}\n\n",
            "class Delegate(inner: Formatter) : Formatter by inner\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("BindModule.kt"),
        concat!(
            "package sample\n\n",
            "@Module\n",
            "abstract class BindModule {\n",
            "    @Binds\n",
            "    abstract fun bindFormatter(target: FormatterImpl): Formatter\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("bindFormatter"),
        "un consommateur-délégant compte comme consommation, stdout:\n{stdout}"
    );
}
