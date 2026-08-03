//! Shapes the tool failed to link back to their symbol.
//!
//! Every test here comes from an empirical probe where the tool called dead
//! some code that runs on every line. The common thread: the symbol's name is
//! written nowhere at the call site, or the declaration was missing from the
//! graph entirely.

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

#[test]
fn operator_conventions_are_called_by_syntax_not_by_name() {
    // All twenty-four operator conventions were reported dead: `a[i]` calls
    // get, `a + b` calls plus, `for (x in c)` calls iterator, `val (a, b) = p`
    // calls component1, `val x by D()` calls getValue. The name never appears
    // at the caller.
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Ops.kt"),
        concat!(
            "package s\n\n",
            "import kotlin.reflect.KProperty\n\n",
            "class Num(val v: Int) {\n",
            "    operator fun plus(o: Num): Num = Num(v + o.v)\n",
            "    operator fun compareTo(o: Num): Int = v.compareTo(o.v)\n",
            "    operator fun unaryMinus(): Num = Num(-v)\n",
            "}\n\n",
            "class Bag(private val items: MutableList<String>) {\n",
            "    operator fun get(i: Int): String = items[i]\n",
            "    operator fun set(i: Int, value: String) { items[i] = value }\n",
            "    operator fun iterator(): Iterator<String> = items.iterator()\n",
            "    operator fun contains(s: String): Boolean = items.contains(s)\n",
            "}\n\n",
            "class Point(private val a: Int, private val b: Int) {\n",
            "    operator fun component1(): Int = a\n",
            "    operator fun component2(): Int = b\n",
            "}\n\n",
            "class Callable {\n",
            "    operator fun invoke(x: Int): Int = x * 2\n",
            "}\n\n",
            "class Deleg {\n",
            "    operator fun getValue(t: Any?, p: KProperty<*>): String = \"x\"\n",
            "    operator fun setValue(t: Any?, p: KProperty<*>, v: String) {}\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        concat!(
            "package s\n\n",
            "var delegated: String by Deleg()\n\n",
            "fun main() {\n",
            "    val n = Num(3)\n",
            "    println(-n)\n",
            "    println(n + Num(1))\n",
            "    if (n < Num(9)) println(\"lt\")\n",
            "    val bag = Bag(mutableListOf(\"a\"))\n",
            "    println(bag[0])\n",
            "    bag[0] = \"b\"\n",
            "    println(\"a\" in bag)\n",
            "    for (item in bag) println(item)\n",
            "    val (x, y) = Point(1, 2)\n",
            "    println(x + y)\n",
            "    println(Callable()(4))\n",
            "    delegated = \"v\"\n",
            "    println(delegated)\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path(), &["--deep"]));
    for operator in [
        "plus",
        "compareTo",
        "unaryMinus",
        "get",
        "set",
        "iterator",
        "contains",
        "component1",
        "component2",
        "invoke",
        "getValue",
        "setValue",
    ] {
        assert!(
            !stdout.contains(&format!("'{operator}'")),
            "`{operator}` is called by syntax, stdout:\n{stdout}"
        );
    }
}

#[test]
fn a_java_record_body_keeps_what_it_uses_alive() {
    // `record_declaration` was not recognised by the parser: neither the
    // record nor its body existed, so everything it used looked dead, and
    // `--delete` removed it, breaking the build.
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Clamp.java"),
        concat!(
            "package app;\n\n",
            "public class Clamp {\n",
            "    public static int nonNegative(int v) { return v < 0 ? 0 : v; }\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Named.java"),
        "package app;\n\npublic interface Named { String label(); }\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Box.java"),
        concat!(
            "package app;\n\n",
            "public record Box(int width, int height) implements Named {\n",
            "    public Box { width = Clamp.nonNegative(width); }\n",
            "    @Override public String label() { return \"box\"; }\n",
            "    public int area() { return width * height; }\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.java"),
        concat!(
            "package app;\n\n",
            "public class Main {\n",
            "    public static void main(String[] a) {\n",
            "        Box b = new Box(a.length, 2);\n",
            "        System.out.println(b.area() + b.label());\n",
            "    }\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path(), &[]));
    assert!(
        !stdout.contains("'Clamp'"),
        "a class called from a record body is alive, stdout:\n{stdout}"
    );
    assert!(
        !stdout.contains("'Named'"),
        "an interface implemented by a record is alive, stdout:\n{stdout}"
    );
}

#[test]
fn a_java_enum_body_is_indexed() {
    // `extract_enum_body` only read the DIRECT children of `enum_body`, but
    // tree-sitter wraps everything past the `;` in `enum_body_declarations`:
    // no Java enum method existed in the graph at all, neither alive nor
    // dead.
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Op.java"),
        concat!(
            "package app;\n\n",
            "public enum Op {\n",
            "    PLUS, MINUS;\n",
            "    public int apply(int a, int b) { return a + b; }\n",
            "    public int neverCalled() { return 0; }\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.java"),
        concat!(
            "package app;\n\n",
            "public class Main {\n",
            "    public static void main(String[] a) {\n",
            "        System.out.println(Op.PLUS.apply(1, 2));\n",
            "        System.out.println(Op.MINUS);\n",
            "    }\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path(), &["--deep"]));
    assert!(
        stdout.contains("neverCalled"),
        "the enum body is indexed, so its dead method shows, stdout:\n{stdout}"
    );
    assert!(
        !stdout.contains("'apply'"),
        "and its called method stays alive, stdout:\n{stdout}"
    );
}

#[test]
fn a_bodyless_type_does_not_swallow_the_next_declaration() {
    // `class Thing` has no body: the brace scan grabbed the one belonging to
    // `fun main`, which was re-parented into the class and lost its
    // entry-point status. The whole file came out dead.
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        concat!(
            "package s\n\n",
            "class Thing\n\n",
            "fun main() {\n",
            "    val t = Thing()\n",
            "    println(t)\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path(), &[]));
    assert!(
        !stdout.contains("'Thing'"),
        "a class used by main in the same file is alive, stdout:\n{stdout}"
    );
}

/// Writes a class, an alias to it, and a `main` that goes ONLY through the
/// alias. One declaration per file: a class sitting next to `main` would
/// measure the bug above instead of the one under test.
fn alias_fixture(dir: &Path, alias_file: &str, main_body: &str) {
    fs::write(dir.join("RealClass.kt"), "package s\n\nclass RealClass\n").unwrap();
    fs::write(dir.join("Alias.kt"), alias_file).unwrap();
    fs::write(
        dir.join("Main.kt"),
        format!("package s\n\nfun main() {{\n    {main_body}\n}}\n"),
    )
    .unwrap();
}

#[test]
fn a_typealias_keeps_the_type_it_names_alive() {
    // `extract_type_alias` looked its name up with
    // `child_by_field_name("simple_identifier")`. tree-sitter-kotlin declares
    // NO field names, so the call always returned None and the alias never
    // entered the graph. Its file then held no declaration at all, so the
    // right-hand-side reference was dropped for lack of a declaration to
    // attribute it to, and the aliased class came out dead.
    let temp = tempfile::tempdir().unwrap();
    alias_fixture(
        temp.path(),
        "package s\n\ntypealias Alias = RealClass\n",
        "val v: Alias? = null\n    println(v)",
    );

    let stdout = stdout_of(&run(temp.path(), &[]));
    assert!(
        !stdout.contains("'RealClass'"),
        "the class is reached through the alias, stdout:\n{stdout}"
    );
}

#[test]
fn a_typealias_chain_is_followed_to_the_end() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("RealClass.kt"),
        "package s\n\nclass RealClass\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("B.kt"),
        "package s\n\ntypealias B = RealClass\n",
    )
    .unwrap();
    fs::write(temp.path().join("A.kt"), "package s\n\ntypealias A = B\n").unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package s\n\nfun main() {\n    println(A())\n}\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path(), &[]));
    assert!(
        !stdout.contains("'RealClass'"),
        "A leads to B which leads to RealClass, stdout:\n{stdout}"
    );
}

#[test]
fn an_unused_typealias_is_reported() {
    // The counterpart of the test above: now that the alias exists as a
    // declaration, an alias nobody uses must show up. Without this assertion
    // we could not tell "the alias resolves" from "the alias went invisible
    // again".
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("RealClass.kt"),
        "package s\n\nclass RealClass\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Alias.kt"),
        "package s\n\ntypealias JamaisUtilise = RealClass\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package s\n\nfun main() {\n    println(RealClass())\n}\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path(), &[]));
    assert!(
        stdout.contains("JamaisUtilise"),
        "an alias nobody uses is dead code, stdout:\n{stdout}"
    );
}

#[test]
fn an_aliased_import_resolves_to_the_type_it_renames() {
    // The resolver already read the "path as alias" form, but
    // `extract_imports` broke out of its loop on the `identifier` node and
    // never looked at the `import_alias` node, so it never fed the resolver
    // one. Side effect measured on a corpus: the alias name fell back to the
    // name index and wrongly retained a same-named symbol from another
    // file.
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("RealClass.kt"),
        "package s\n\nclass RealClass\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package t\n\nimport s.RealClass as Bar\n\nfun main() {\n    println(Bar())\n}\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path(), &[]));
    assert!(
        !stdout.contains("'RealClass'"),
        "Bar names RealClass, stdout:\n{stdout}"
    );
}

#[test]
fn dc003_sees_an_unused_kotlin_parameter() {
    // DC003 worked in Java and not at all in Kotlin: `extract_parameters` was
    // only reached through `child_by_field_name`, which this grammar never
    // answers, so no Kotlin parameter existed in the graph. Same fixture, two
    // languages, two verdicts.
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Compute.kt"),
        "package s\n\nfun compute(used: Int, neverRead: String): Int {\n    return used * 2\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package s\n\nfun main() {\n    println(compute(1, \"x\"))\n}\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path(), &[]));
    assert!(
        stdout.contains("neverRead"),
        "a Kotlin parameter nobody reads is DC003, stdout:\n{stdout}"
    );
    assert!(
        !stdout.contains("'used'"),
        "and the one the body reads is not, stdout:\n{stdout}"
    );
}

#[test]
fn a_parameter_does_not_answer_for_its_namesake_elsewhere() {
    // Simple-name resolution is global. Without scoping, `shared` read in `f`
    // bound to EVERY parameter named `shared` in the project, so the unused one
    // in `g` looked used. Worse: that parameter went reachable, and since
    // reachability marks ancestors, it resurrected `g` and its whole class.
    // Measured on Java before any Kotlin parameter existed, so the flaw
    // predates the Kotlin side.
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("A.kt"),
        "package s\n\nfun f(shared: Int): Int = shared * 2\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("B.kt"),
        "package s\n\nfun g(shared: Int): Int = 7\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package s\n\nfun main() {\n    println(f(1) + g(2))\n}\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path(), &[]));
    assert!(
        stdout.contains("DC003"),
        "the unused `shared` of g must not hide behind the used one of f, stdout:\n{stdout}"
    );
}

#[test]
fn a_default_value_still_counts_as_a_use() {
    // The parameter's own name and its default value are both
    // `simple_identifier` children of the same `parameter` node. Silencing the
    // name must not silence the default value, or `MY_CONST` would go dead.
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Const.kt"),
        "package s\n\nconst val MY_CONST: Int = 7\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Test.kt"),
        "package s\n\nfun test(x: Int = MY_CONST): Int = x\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package s\n\nfun main() {\n    println(test())\n}\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path(), &[]));
    assert!(
        !stdout.contains("MY_CONST"),
        "a constant used as a default value is alive, stdout:\n{stdout}"
    );
}

#[test]
fn an_operator_keeps_the_parameters_the_language_imposes() {
    // A property delegate must take `thisRef` and `property` whether it reads
    // them or not. Reporting them is not actionable: they cannot be removed.
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Deleg.kt"),
        concat!(
            "package s\n\n",
            "import kotlin.reflect.KProperty\n\n",
            "class Deleg {\n",
            "    operator fun getValue(t: Any?, p: KProperty<*>): String = \"x\"\n",
            "    operator fun setValue(t: Any?, p: KProperty<*>, v: String) {}\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        concat!(
            "package s\n\n",
            "var delegated: String by Deleg()\n\n",
            "fun main() {\n",
            "    delegated = \"v\"\n",
            "    println(delegated)\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path(), &[]));
    assert!(
        !stdout.contains("DC003"),
        "an imposed signature is not a finding, stdout:\n{stdout}"
    );
}

#[test]
fn suppress_on_the_parameter_or_its_function_silences_dc003() {
    // The developer already answered this diagnostic by name. The parameter's
    // own annotation sits in a `parameter_modifiers` SIBLING node, which the
    // extractor used to walk right past.
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("T.kt"),
        concat!(
            "package s\n\n",
            "class T {\n",
            "    fun onParam(a: Int, @Suppress(\"UNUSED_PARAMETER\") b: Int): Int = a\n",
            "    @Suppress(\"UNUSED_PARAMETER\")\n",
            "    fun onFunction(a: Int, b: Int): Int = a\n",
            "    fun witness(a: Int, stillReported: Int): Int = a\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        concat!(
            "package s\n\n",
            "fun main() {\n",
            "    val t = T()\n",
            "    println(t.onParam(1, 2) + t.onFunction(3, 4) + t.witness(5, 6))\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path(), &[]));
    assert!(
        !stdout.contains("'b'"),
        "both suppressed parameters stay silent, stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("stillReported"),
        "and the un-suppressed witness still fires, stdout:\n{stdout}"
    );
}

#[test]
fn main_parameters_are_an_imposed_signature() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package s\n\nfun main(args: Array<String>) {\n    println(\"x\")\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.java"),
        concat!(
            "package app;\n",
            "public class Main {\n",
            "    public static void main(String[] args) { System.out.println(\"x\"); }\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path(), &[]));
    assert!(
        !stdout.contains("DC003"),
        "the JVM requires main's signature, stdout:\n{stdout}"
    );
}

#[test]
fn an_aliased_import_of_a_nested_member_resolves_by_its_last_segment() {
    // `import a.Outer.Inner as Bar`: the FQN index keys members by their own
    // path, not by the dotted access path of the import, so the exact lookup
    // misses and the alias resolved to nothing. Nested classes, object
    // members and sealed variants were all reported dead — measured on the
    // published 0.15.1, before the import-alias fix existed.
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Outer.kt"),
        "package a\n\nclass Outer {\n    class Inner\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Holder.kt"),
        "package a\n\nobject Holder {\n    fun helper(): Int = 1\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Ev.kt"),
        "package a\n\nsealed class Ev {\n    class Tap : Ev()\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        concat!(
            "package t\n\n",
            "import a.Outer.Inner as Bar\n",
            "import a.Holder.helper as h\n",
            "import a.Ev.Tap as T\n\n",
            "fun main() {\n",
            "    println(Bar())\n",
            "    println(h())\n",
            "    println(T())\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path(), &[]));
    for symbol in [
        "'Inner'", "'Outer'", "'Holder'", "'helper'", "'Tap'", "'Ev'",
    ] {
        assert!(
            !stdout.contains(symbol),
            "{symbol} is reached through its aliased import, stdout:\n{stdout}"
        );
    }
}

#[test]
fn an_annotated_bodyless_class_does_not_lend_its_suppress_to_a_neighbour() {
    // The grammar parses `@Suppress("unused") class Annotated` (bodyless)
    // into a prefix_expression that swallows the following bodyless
    // declarations. Capturing that expression's text wholesale handed the
    // `@Suppress` to the NEXT real declaration, which silently vanished
    // from the report.
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Mix.kt"),
        concat!(
            "package s\n\n",
            "@Suppress(\"unused\")\n",
            "class Annotated\n\n",
            "class Plain\n\n",
            "abstract class Orphan {\n",
            "    abstract fun f()\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package s\n\nfun main() {\n    println(Plain())\n}\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path(), &["--deep"]));
    assert!(
        stdout.contains("'Orphan'"),
        "the suppression belongs to Annotated, not to Orphan, stdout:\n{stdout}"
    );
    assert!(
        !stdout.contains("'Annotated'"),
        "and Annotated keeps its own @Suppress, stdout:\n{stdout}"
    );
}
