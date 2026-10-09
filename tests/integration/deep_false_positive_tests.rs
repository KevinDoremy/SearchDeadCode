//! Faux positifs attrapés en nettoyant un vrai monorepo Android (replica +
//! rubicon) avec le deep analyzer. Chaque test rejoue le cas minimal et
//! vérifie que le symbole vivant n'est plus signalé, avec un témoin mort
//! à côté pour prouver que le détecteur fonctionne toujours.

use searchdeadcode::analysis::{DeadCode, DeepAnalyzer, EntryPointDetector};
use searchdeadcode::config::Config;
use searchdeadcode::discovery::{FileType, SourceFile};
use searchdeadcode::graph::{Graph, GraphBuilder};
use std::path::Path;

const MANIFEST: &str = r#"<manifest xmlns:android="http://schemas.android.com/apk/res/android" package="com.fp">
    <application>
        <activity android:name=".MainActivity" />
    </application>
</manifest>"#;

/// Writes the files under a temp root (paths relative to it, so source sets
/// like `app/src/debug/...` are real) and runs the deep pass on them.
fn deep_findings(files: &[(&str, &str)]) -> (Vec<DeadCode>, Graph) {
    let temp = tempfile::tempdir().expect("temp dir");
    let root = temp.path();
    let mut builder = GraphBuilder::new();
    std::fs::create_dir_all(root.join("app/src/main")).unwrap();
    std::fs::write(root.join("app/src/main/AndroidManifest.xml"), MANIFEST).unwrap();
    for (relative, content) in files {
        let path = root.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, content).unwrap();
        let file_type = match Path::new(relative).extension().and_then(|e| e.to_str()) {
            Some("java") => FileType::Java,
            Some("kt") => FileType::Kotlin,
            // Layouts and other resources are written for the entry-point
            // detector to find, never parsed as code.
            _ => continue,
        };
        builder
            .process_file(&SourceFile::new(path, file_type))
            .expect("parse");
    }
    let graph = builder.build();
    let config = Config::default();
    let entry_points = EntryPointDetector::new(&config)
        .detect(&graph, root)
        .expect("entry points");
    let analyzer = DeepAnalyzer::new().with_unused_members(true);
    let (dead, _) = analyzer.analyze(&graph, &entry_points);
    (dead, graph)
}

fn reported(dead: &[DeadCode], name: &str) -> Vec<String> {
    dead.iter()
        .filter(|d| d.declaration.name == name)
        .map(|d| d.message.clone())
        .collect()
}

const ACTIVITY_HEAD: &str = r#"package com.fp

import android.app.Activity
import android.os.Bundle
"#;

#[test]
fn a_member_import_keeps_its_container_alive() {
    let (dead, _) = deep_findings(&[
        (
            "app/src/main/java/com/fp/MainActivity.kt",
            &format!(
                "{ACTIVITY_HEAD}import com.fp.InlineModelDelegate.Companion.cleanRangeValues
import com.fp.NetworkUtils.isOnWifi

class MainActivity : Activity() {{
    override fun onCreate(savedInstanceState: Bundle?) {{
        super.onCreate(savedInstanceState)
        println(cleanRangeValues(1, 2))
        println(isOnWifi(this))
    }}
}}
"
            ),
        ),
        (
            "app/src/main/java/com/fp/InlineModelDelegate.kt",
            r#"package com.fp

class InlineModelDelegate private constructor() {
    companion object {
        fun cleanRangeValues(start: Int, end: Int): Int = start + end
    }
}
"#,
        ),
        (
            "app/src/main/java/com/fp/NetworkUtils.kt",
            r#"package com.fp

import android.content.Context

object NetworkUtils {
    fun isOnWifi(context: Context): Boolean = context.hashCode() % 2 == 0
}

object OrphanUtils {
    fun unused(): Int = 1
}
"#,
        ),
    ]);
    assert!(
        reported(&dead, "InlineModelDelegate").is_empty(),
        "the companion member is imported: {dead:?}"
    );
    assert!(
        reported(&dead, "NetworkUtils").is_empty(),
        "the object member is imported: {dead:?}"
    );
    assert!(
        !reported(&dead, "OrphanUtils").is_empty(),
        "the control object nobody imports must still be reported"
    );
}

#[test]
fn enum_entries_reached_by_iteration_keep_their_initializers_alive() {
    let (dead, _) = deep_findings(&[
        (
            "app/src/main/java/com/fp/MainActivity.kt",
            &format!(
                "{ACTIVITY_HEAD}
class MainActivity : Activity() {{
    override fun onCreate(savedInstanceState: Bundle?) {{
        super.onCreate(savedInstanceState)
        OnboardingBackground()
    }}
}}
"
            ),
        ),
        (
            "app/src/main/java/com/fp/OnboardingBackground.kt",
            r#"package com.fp

fun OnboardingBackground() {
    OnboardingBackgroundCircle.entries.forEach { circle -> println(circle.radius) }
}

private fun constantValue(value: Float): Array<Float> = arrayOf(value, value)

private fun neverCalled(value: Float): Float = value

private enum class OnboardingBackgroundCircle(val radius: Array<Float>) {
    FIXED_RED(radius = constantValue(0.4F)),
    FIXED_BLUE(radius = constantValue(0.2F)),
}
"#,
        ),
    ]);
    assert!(
        reported(&dead, "constantValue").is_empty(),
        "called from the entries' constructor arguments: {dead:?}"
    );
    assert!(
        reported(&dead, "FIXED_RED").is_empty() && reported(&dead, "FIXED_BLUE").is_empty(),
        "entries of an iterated enum are all reached: {dead:?}"
    );
    assert!(
        !reported(&dead, "neverCalled").is_empty(),
        "the control helper nobody calls must still be reported"
    );
}

#[test]
fn a_custom_accessor_is_not_write_only_but_a_plain_private_var_is() {
    let (dead, _) = deep_findings(&[
        (
            "app/src/main/java/com/fp/MainActivity.kt",
            &format!(
                "{ACTIVITY_HEAD}
class MainActivity : Activity() {{
    override fun onCreate(savedInstanceState: Bundle?) {{
        super.onCreate(savedInstanceState)
        Watchdog().restart()
    }}
}}
"
            ),
        ),
        (
            "app/src/main/java/com/fp/Watchdog.kt",
            r#"package com.fp

class Job {
    fun cancel() {}
}

class Watchdog {
    private var stallWatchdogJob: Job? = null
        set(value) {
            field?.cancel()
            field = value
        }

    private var restartCount = 0

    fun restart() {
        stallWatchdogJob = Job()
        stallWatchdogJob = null
        restartCount = restartCount + 0
        restartCount = 1
    }
}
"#,
        ),
    ]);
    assert!(
        reported(&dead, "stallWatchdogJob").is_empty(),
        "the setter reads `field` on its own: {dead:?}"
    );
}

#[test]
fn a_property_of_a_test_source_set_is_not_write_only() {
    let (dead, _) = deep_findings(&[
        (
            "app/src/main/java/com/fp/Controller.kt",
            "package com.fp\n\nclass Controller { init { println(\"activate\") } }\n",
        ),
        (
            "app/src/test/java/com/fp/ControllerTest.kt",
            r#"package com.fp

import org.junit.Before
import org.junit.Test

class ControllerTest {
    private lateinit var controller: Controller

    @Before
    fun setup() {
        controller = Controller()
    }

    @Test
    fun `activates on construction`() {
        println("verify")
    }
}
"#,
        ),
    ]);
    assert!(
        reported(&dead, "controller").is_empty(),
        "a test holds the object under test for its side effects: {dead:?}"
    );
}

#[test]
fn debug_source_set_patterns_skip_parameters_and_referenced_symbols() {
    let (dead, _) = deep_findings(&[
        (
            "app/src/debug/java/com/fp/di/LogModule.kt",
            r#"package com.fp.di

import dagger.Binds
import dagger.Module

open class Tree
class HyperlinkedDebugTree : Tree()

@Module
abstract class LogModule {
    @Binds
    abstract fun bindTimberTree(implementation: HyperlinkedDebugTree): Tree
}
"#,
        ),
        (
            "app/src/debug/java/com/fp/preview/GridGamePreview.kt",
            r#"package com.fp.preview

import androidx.compose.runtime.Composable
import androidx.compose.ui.tooling.preview.Preview

@Preview(name = "Light")
@Composable
private fun GridGameOverlayLightPreview() {
    GridGameOverlayPreviewContent()
}

@Composable
private fun GridGameOverlayPreviewContent(description: String = "Reprends une partie") {
    println(description)
}

@Composable
private fun NobodyPreviewsMe() {
    println("dead")
}
"#,
        ),
    ]);
    assert!(
        reported(&dead, "implementation").is_empty(),
        "a @Binds parameter is read by Dagger, not by a pattern: {dead:?}"
    );
    assert!(
        reported(&dead, "GridGameOverlayPreviewContent").is_empty(),
        "called by its @Preview: {dead:?}"
    );
    assert!(
        reported(&dead, "description").is_empty(),
        "a parameter is DC003's business, never a debug-only finding: {dead:?}"
    );
    let control = reported(&dead, "NobodyPreviewsMe");
    assert!(
        control.iter().any(|m| m.contains("debug-only")),
        "an unreferenced function of the debug source set is still debug-only: {control:?}"
    );
}

#[test]
fn a_package_directory_named_debug_is_not_a_debug_source_set() {
    let (dead, _) = deep_findings(&[(
        "app/src/main/java/com/fp/debug/ShortcutHelper.kt",
        r#"package com.fp.debug

object ShortcutHelper {
    fun initializeShortcuts(): Int = 1
}
"#,
    )]);
    let messages = reported(&dead, "ShortcutHelper");
    assert!(
        messages.iter().all(|m| !m.contains("debug-only")),
        "src/main code in a package named debug ships in release: {messages:?}"
    );
}

#[test]
fn two_annotations_stacked_before_a_modifier_still_declare_the_function() {
    let (dead, graph) = deep_findings(&[(
        "app/src/main/java/com/fp/Stacked.kt",
        r#"package com.fp

annotation class A
annotation class B

@A
@B
private fun TwoAnnotsPrivate() {
    Callee()
}

@A @B internal fun TwoAnnotsSameLine() {
    Callee()
}

private fun Callee() {}
"#,
    )]);
    let names: Vec<&str> = graph
        .declarations()
        .filter(|d| d.name.starts_with("TwoAnnots"))
        .map(|d| d.name.as_str())
        .collect();
    assert!(
        names.contains(&"TwoAnnotsPrivate") && names.contains(&"TwoAnnotsSameLine"),
        "both garbled declarations are recovered: {names:?}"
    );
    let recovered = graph
        .declarations()
        .find(|d| d.name == "TwoAnnotsPrivate")
        .expect("recovered");
    assert!(
        recovered.annotations.iter().any(|a| a.contains("@A")),
        "the recovered declaration gets its annotations back: {:?}",
        recovered.annotations
    );
    assert!(
        reported(&dead, "Callee")
            .iter()
            .all(|m| !m.contains("never used")),
        "the recovered body references its callee: {dead:?}"
    );
}

// ---------------------------------------------------------------------------
// Second batch, same monorepo, same day: Java accessors reached from Kotlin
// without a receiver, abstract constructors, and in-class private reads.
// ---------------------------------------------------------------------------

#[test]
fn inherited_java_getter_read_as_a_bare_kotlin_property_is_alive() {
    let (dead, _) = deep_findings(&[
        (
            "app/src/main/java/com/fp/base/Helper.java",
            "package com.fp.base;\n\npublic class Helper {\n    public void begin() {}\n}\n",
        ),
        (
            "app/src/main/java/com/fp/base/BaseActivity.java",
            r#"package com.fp.base;

import android.app.Activity;

public abstract class BaseActivity extends Activity {
    Helper helper;

    protected Helper getHelper() {
        return helper;
    }

    protected Helper getUnusedHelper() {
        return helper;
    }
}
"#,
        ),
        (
            "app/src/main/java/com/fp/MainActivity.kt",
            r#"package com.fp

import android.os.Bundle
import com.fp.base.BaseActivity

class MainActivity : BaseActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        helper.begin()
    }
}
"#,
        ),
    ]);
    assert!(
        reported(&dead, "getHelper").is_empty(),
        "`helper.begin()` in the Kotlin subclass compiles to getHelper(): {:?}",
        reported(&dead, "getHelper")
    );
    assert!(
        !reported(&dead, "getUnusedHelper").is_empty(),
        "the getter nobody reads must still be reported"
    );
}

#[test]
fn kotlin_property_write_needs_the_java_getter_too() {
    let (dead, _) = deep_findings(&[
        (
            "app/src/main/java/com/fp/ScrollView.java",
            r#"package com.fp;

public class ScrollView {
    private Object fadeLayer;

    public void setFadeLayer(Object fadeLayer) {
        this.fadeLayer = fadeLayer;
    }

    public Object getFadeLayer() {
        return fadeLayer;
    }

    public Object getOther() {
        return null;
    }
}
"#,
        ),
        (
            "app/src/main/java/com/fp/MainActivity.kt",
            &format!(
                "{ACTIVITY_HEAD}
class MainActivity : Activity() {{
    override fun onCreate(savedInstanceState: Bundle?) {{
        super.onCreate(savedInstanceState)
        val view = ScrollView()
        view.fadeLayer = Any()
    }}
}}
"
            ),
        ),
    ]);
    assert!(
        reported(&dead, "getFadeLayer").is_empty(),
        "`view.fadeLayer = x` only compiles while getFadeLayer() exists: {:?}",
        reported(&dead, "getFadeLayer")
    );
    assert!(
        !reported(&dead, "getOther").is_empty(),
        "the accessor nobody touches must still be reported"
    );
}

#[test]
fn abstract_java_constructors_called_by_super_and_anonymous_subclasses_are_alive() {
    let (dead, _) = deep_findings(&[
        (
            "app/src/main/java/com/fp/BaseShare.java",
            r#"package com.fp;

public abstract class BaseShare {
    protected final Object context;

    public BaseShare(final Object context) {
        this.context = context;
    }

    public abstract void launch();
}
"#,
        ),
        (
            "app/src/main/java/com/fp/LeakSafeRunnable.java",
            r#"package com.fp;

public abstract class LeakSafeRunnable<T extends Object> implements Runnable {
    private final T target;

    public LeakSafeRunnable(T target) {
        this.target = target;
    }

    @Override
    public final void run() {
        doWork(target);
    }

    public abstract void doWork(T target);
}
"#,
        ),
        (
            "app/src/main/java/com/fp/EditionShare.kt",
            "package com.fp\n\nclass EditionShare(activity: Any) : BaseShare(activity) {\n    override fun launch() {\n        println(context)\n    }\n}\n",
        ),
        (
            "app/src/main/java/com/fp/MainActivity.kt",
            &format!(
                "{ACTIVITY_HEAD}
class MainActivity : Activity() {{
    override fun onCreate(savedInstanceState: Bundle?) {{
        super.onCreate(savedInstanceState)
        EditionShare(this).launch()
        val runnable = object : LeakSafeRunnable<MainActivity>(this) {{
            override fun doWork(target: MainActivity) {{
                println(target)
            }}
        }}
        runnable.run()
    }}
}}
"
            ),
        ),
    ]);
    assert!(
        reported(&dead, "BaseShare").is_empty(),
        "`: BaseShare(activity)` calls the constructor: {:?}",
        reported(&dead, "BaseShare")
    );
    assert!(
        reported(&dead, "LeakSafeRunnable").is_empty(),
        "`object : LeakSafeRunnable<T>(this)` calls the constructor: {:?}",
        reported(&dead, "LeakSafeRunnable")
    );
}

#[test]
fn java_diamond_instantiation_calls_the_constructor() {
    let (dead, _) = deep_findings(&[
        (
            "app/src/main/java/com/fp/Gallery.java",
            r#"package com.fp;

import java.util.List;

public class Gallery<P> {
    private final List<P> models;

    public Gallery(final Object controller, List<P> models) {
        this.models = models;
    }

    public int size() {
        return models.size();
    }
}
"#,
        ),
        (
            "app/src/main/java/com/fp/GalleryUtils.java",
            r#"package com.fp;

import java.util.ArrayList;

public final class GalleryUtils {
    public static int build(final Object controller) {
        final Gallery<String> gallery = new Gallery<>(controller, new ArrayList<>());
        return gallery.size();
    }
}
"#,
        ),
        (
            "app/src/main/java/com/fp/MainActivity.kt",
            &format!(
                "{ACTIVITY_HEAD}
class MainActivity : Activity() {{
    override fun onCreate(savedInstanceState: Bundle?) {{
        super.onCreate(savedInstanceState)
        println(GalleryUtils.build(this))
    }}
}}
"
            ),
        ),
    ]);
    assert!(
        reported(&dead, "Gallery").is_empty(),
        "`new Gallery<>(...)` is an instantiation: {:?}",
        reported(&dead, "Gallery")
    );
}

#[test]
fn private_val_shadowing_an_imported_extension_is_read_in_its_class() {
    let (dead, _) = deep_findings(&[
        (
            "app/src/main/java/com/fp/utils/ContextExt.kt",
            "package com.fp.utils\n\nimport android.content.Context\n\nval Context.isInTestLabMode: Boolean\n    get() = true\n",
        ),
        (
            "app/src/main/java/com/fp/AnalyticsManager.kt",
            r#"package com.fp

import android.content.Context
import com.fp.utils.isInTestLabMode

class AnalyticsManager(context: Context) {
    private val isInTestLabMode = context.isInTestLabMode
    private val neverRead = context.isInTestLabMode

    fun track() {
        if (isInTestLabMode) return
        println("tracked")
    }
}
"#,
        ),
        (
            "app/src/main/java/com/fp/MainActivity.kt",
            &format!(
                "{ACTIVITY_HEAD}
class MainActivity : Activity() {{
    override fun onCreate(savedInstanceState: Bundle?) {{
        super.onCreate(savedInstanceState)
        AnalyticsManager(this).track()
    }}
}}
"
            ),
        ),
    ]);
    assert!(
        reported(&dead, "isInTestLabMode").is_empty(),
        "the private val is read by track(), the extension by its initializer: {:?}",
        reported(&dead, "isInTestLabMode")
    );
    assert!(
        !reported(&dead, "neverRead").is_empty(),
        "the sibling nobody reads must still be reported"
    );
}

#[test]
fn top_level_val_passed_as_a_named_argument_is_read() {
    let (dead, _) = deep_findings(&[
        (
            "app/src/main/java/com/fp/theme/Type.kt",
            "package com.fp.theme\n\nclass Typography\n\nval AppTypography = Typography()\n\nval UnusedTypography = Typography()\n",
        ),
        (
            "app/src/main/java/com/fp/theme/Theme.kt",
            "package com.fp.theme\n\nfun materialTheme(typography: Typography) {\n    println(typography)\n}\n\nfun applyTheme() {\n    materialTheme(\n        typography = AppTypography,\n    )\n}\n",
        ),
        (
            "app/src/main/java/com/fp/MainActivity.kt",
            &format!(
                "{ACTIVITY_HEAD}import com.fp.theme.applyTheme

class MainActivity : Activity() {{
    override fun onCreate(savedInstanceState: Bundle?) {{
        super.onCreate(savedInstanceState)
        applyTheme()
    }}
}}
"
            ),
        ),
    ]);
    assert!(
        reported(&dead, "AppTypography").is_empty(),
        "`typography = AppTypography` reads the val: {:?}",
        reported(&dead, "AppTypography")
    );
    assert!(
        !reported(&dead, "UnusedTypography").is_empty(),
        "the val nobody reads must still be reported"
    );
}

#[test]
fn package_private_setter_called_from_a_chained_java_instantiation_is_alive() {
    let (dead, _) = deep_findings(&[
        (
            "app/src/main/java/com/fp/Cell.java",
            "package com.fp;\n\nclass Cell {\n    private float highlightAlpha;\n\n    float getHighlightAlpha() {\n        return highlightAlpha;\n    }\n\n    void setHighlightAlpha(float value) {\n        this.highlightAlpha = value;\n    }\n\n    void setNeverCalled(float value) {\n        this.highlightAlpha = value;\n    }\n}\n",
        ),
        (
            "app/src/main/java/com/fp/CellAnimator.java",
            "package com.fp;\n\nimport java.util.List;\n\nclass CellAnimator {\n    private final List<Cell> cells;\n\n    CellAnimator(final Object view, final List<Cell> cells) {\n        this.cells = cells;\n    }\n\n    void execute() {\n        for (final Cell cell : cells) {\n            cell.setHighlightAlpha(cell.getHighlightAlpha() + 1f);\n        }\n    }\n}\n",
        ),
        (
            "app/src/main/java/com/fp/GridView.java",
            "package com.fp;\n\nimport java.util.List;\n\npublic class GridView {\n    public void highlight(final List<Cell> cells) {\n        new CellAnimator(this, cells).execute();\n    }\n}\n",
        ),
        (
            "app/src/main/java/com/fp/MainActivity.kt",
            &format!(
                "{ACTIVITY_HEAD}
class MainActivity : Activity() {{
    override fun onCreate(savedInstanceState: Bundle?) {{
        super.onCreate(savedInstanceState)
        GridView().highlight(emptyList())
    }}
}}
"
            ),
        ),
    ]);
    assert!(
        reported(&dead, "CellAnimator").is_empty(),
        "`new CellAnimator(...).execute()` instantiates it: {:?}",
        reported(&dead, "CellAnimator")
    );
    assert!(
        reported(&dead, "setHighlightAlpha").is_empty(),
        "called from execute(): {:?}",
        reported(&dead, "setHighlightAlpha")
    );
    assert!(
        !reported(&dead, "setNeverCalled").is_empty(),
        "the setter nobody calls must still be reported"
    );
}

/// The CLI builds its graph with the parallel builder; the harness above uses
/// the serial one. A top-level val read as a named argument resolved on the
/// serial path and came out with zero incoming edges on the parallel path.
#[test]
fn parallel_builder_resolves_a_top_level_val_read_as_named_argument() {
    use searchdeadcode::graph::ParallelGraphBuilder;
    let temp = tempfile::tempdir().expect("temp dir");
    let root = temp.path();
    let files = [
        (
            "app/src/main/java/com/fp/theme/Type.kt",
            "package com.fp.theme\n\nclass Typography\n\nval AppTypography = Typography()\n",
        ),
        (
            "app/src/main/java/com/fp/theme/Theme.kt",
            "package com.fp.theme\n\nfun materialTheme(typography: Typography) {\n    println(typography)\n}\n\nfun applyTheme() {\n    materialTheme(\n        typography = AppTypography,\n    )\n}\n",
        ),
    ];
    let mut sources = Vec::new();
    for (relative, content) in files {
        let path = root.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, content).unwrap();
        sources.push(SourceFile::new(path, FileType::Kotlin));
    }
    let graph = ParallelGraphBuilder::new()
        .build_from_files(&sources)
        .expect("graph");
    let app_typography = graph
        .find_by_name("AppTypography")
        .into_iter()
        .next()
        .expect("declared");
    assert!(
        graph.is_referenced(&app_typography.id),
        "`typography = AppTypography` must reach the val on the parallel path too"
    );
}

#[test]
fn material_theme_builder_files_are_retained_not_excluded() {
    let (dead, _) = deep_findings(&[
        (
            "app/src/main/java/com/fp/ui/theme/Type.kt",
            "package com.fp.ui.theme\n\nclass Typography\n\nval AppTypography = Typography()\n\nval UnusedTypography = Typography()\n",
        ),
        (
            "app/src/main/java/com/fp/ui/theme/Color.kt",
            "package com.fp.ui.theme\n\nval purple80 = 0xFFD0BCFF\n",
        ),
        (
            "app/src/main/java/com/fp/ui/theme/Theme.kt",
            "package com.fp.ui.theme\n\nfun materialTheme(typography: Typography) {\n    println(typography)\n}\n\nfun AppTheme() {\n    materialTheme(typography = AppTypography)\n}\n",
        ),
        (
            "app/src/main/java/com/fp/MainActivity.kt",
            &format!(
                "{ACTIVITY_HEAD}
class MainActivity : Activity() {{
    override fun onCreate(savedInstanceState: Bundle?) {{
        super.onCreate(savedInstanceState)
    }}
}}
"
            ),
        ),
    ]);
    assert!(
        reported(&dead, "AppTypography").is_empty(),
        "Theme.kt is retained, so what it reads stays alive: {:?}",
        reported(&dead, "AppTypography")
    );
    assert!(
        reported(&dead, "purple80").is_empty(),
        "a palette defines every tone on purpose: {:?}",
        reported(&dead, "purple80")
    );
    assert!(
        reported(&dead, "AppTheme").is_empty(),
        "declarations of a retained file are never reported: {:?}",
        reported(&dead, "AppTheme")
    );
    assert!(
        !reported(&dead, "UnusedTypography").is_empty(),
        "Type.kt is an ordinary file: its unused val is still reported"
    );
}

#[test]
fn object_animator_property_names_reach_their_accessors_from_java() {
    let (dead, _) = deep_findings(&[
        (
            "app/src/main/java/com/fp/Cell.java",
            "package com.fp;\n\nclass Cell {\n    private float highlightAlpha;\n\n    void setHighlightAlpha(float value) {\n        this.highlightAlpha = value;\n    }\n\n    void setNeverCalled(float value) {\n        this.highlightAlpha = value;\n    }\n}\n",
        ),
        (
            "app/src/main/java/com/fp/CellAnimator.java",
            r#"package com.fp;

import android.animation.ObjectAnimator;

public class CellAnimator {
    private final Cell cell = new Cell();

    public void execute() {
        ObjectAnimator.ofFloat(this, "cellAlpha", 1F, 0F).start();
    }

    @SuppressWarnings("unused")
    public void setCellAlpha(float value) {
        cell.setHighlightAlpha(value);
    }
}
"#,
        ),
        (
            "app/src/main/java/com/fp/MainActivity.kt",
            &format!(
                "{ACTIVITY_HEAD}
class MainActivity : Activity() {{
    override fun onCreate(savedInstanceState: Bundle?) {{
        super.onCreate(savedInstanceState)
        CellAnimator().execute()
    }}
}}
"
            ),
        ),
    ]);
    assert!(
        reported(&dead, "setCellAlpha").is_empty(),
        "`ofFloat(this, \"cellAlpha\", …)` drives setCellAlpha() by reflection: {:?}",
        reported(&dead, "setCellAlpha")
    );
    assert!(
        reported(&dead, "setHighlightAlpha").is_empty(),
        "called from the reflectively reached setter: {:?}",
        reported(&dead, "setHighlightAlpha")
    );
    assert!(
        !reported(&dead, "setNeverCalled").is_empty(),
        "the setter nobody names must still be reported"
    );
}

#[test]
fn object_animator_property_names_reach_their_accessors_from_kotlin() {
    let (dead, _) = deep_findings(&[
        (
            "app/src/main/java/com/fp/Fader.kt",
            r#"package com.fp

import android.animation.ObjectAnimator

class Fader {
    private var alpha = 1F
    private var neverSet = 1F

    fun execute() {
        ObjectAnimator.ofFloat(this, "cellAlpha", 1F, 0F).start()
    }

    @Suppress("unused")
    fun setCellAlpha(value: Float) {
        alpha = value
    }

    fun setOther(value: Float) {
        neverSet = value
    }
}
"#,
        ),
        (
            "app/src/main/java/com/fp/MainActivity.kt",
            &format!(
                "{ACTIVITY_HEAD}
class MainActivity : Activity() {{
    override fun onCreate(savedInstanceState: Bundle?) {{
        super.onCreate(savedInstanceState)
        Fader().execute()
    }}
}}
"
            ),
        ),
    ]);
    assert!(
        reported(&dead, "setCellAlpha").is_empty(),
        "`ofFloat(this, \"cellAlpha\", …)` drives setCellAlpha() by reflection: {:?}",
        reported(&dead, "setCellAlpha")
    );
    assert!(
        !reported(&dead, "setOther").is_empty(),
        "the setter nobody names must still be reported"
    );
}

#[test]
fn a_view_known_only_to_a_dead_layout_and_a_di_inject_is_dead() {
    let (dead, _) = deep_findings(&[
        (
            "app/src/main/res/layout/widget_panel.xml",
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<com.fp.Panel xmlns:android=\"http://schemas.android.com/apk/res/android\"\n    android:layout_width=\"match_parent\"\n    android:layout_height=\"match_parent\" />\n",
        ),
        (
            "app/src/main/java/com/fp/Panel.kt",
            "package com.fp\n\nimport android.content.Context\nimport android.view.View\n\nclass Panel(context: Context) : View(context)\n",
        ),
        (
            "app/src/main/java/com/fp/Used.kt",
            "package com.fp\n\nclass Used\n",
        ),
        (
            "app/src/main/java/com/fp/AppComponent.kt",
            r#"package com.fp

import dagger.Component

@Component
interface AppComponent {
    fun inject(target: Panel)
    fun inject(target: Used)
}

class EmptyAppComponent : AppComponent {
    override fun inject(target: Panel) {}
    override fun inject(target: Used) {}
}
"#,
        ),
        (
            "app/src/main/java/com/fp/MainActivity.kt",
            &format!(
                "{ACTIVITY_HEAD}
class MainActivity : Activity() {{
    private val component: AppComponent = EmptyAppComponent()

    override fun onCreate(savedInstanceState: Bundle?) {{
        super.onCreate(savedInstanceState)
        component.inject(Used())
    }}
}}
"
            ),
        ),
    ]);
    assert!(
        !reported(&dead, "Panel").is_empty(),
        "nothing inflates widget_panel.xml and `inject(target: Panel)` only exists because Panel asked: Panel is dead"
    );
    assert!(
        reported(&dead, "Used").is_empty(),
        "instantiated by the activity: {:?}",
        reported(&dead, "Used")
    );
}

#[test]
fn kotlin_secondary_constructor_super_call_reaches_the_java_constructor() {
    let (dead, _) = deep_findings(&[
        (
            "app/src/main/java/com/fp/CellView.java",
            r#"package com.fp;

import android.content.Context;
import android.util.AttributeSet;
import android.view.View;

public abstract class CellView extends View {
    public CellView(Context context) {
        super(context);
    }

    public CellView(Context context, AttributeSet attrs) {
        super(context, attrs);
    }
}
"#,
        ),
        (
            "app/src/main/java/com/fp/SudokuCellView.kt",
            r#"package com.fp

import android.content.Context
import android.util.AttributeSet

class SudokuCellView : CellView {
    constructor(context: Context) : this(context, null)

    constructor(context: Context, attrs: AttributeSet?) : super(context, attrs)
}
"#,
        ),
        (
            "app/src/main/java/com/fp/MainActivity.kt",
            &format!(
                "{ACTIVITY_HEAD}
class MainActivity : Activity() {{
    override fun onCreate(savedInstanceState: Bundle?) {{
        super.onCreate(savedInstanceState)
        println(SudokuCellView(this))
    }}
}}
"
            ),
        ),
    ]);
    assert!(
        reported(&dead, "CellView").is_empty(),
        "`: super(context, attrs)` calls the Java constructor: {:?}",
        reported(&dead, "CellView")
    );
    assert!(
        reported(&dead, "SudokuCellView").is_empty(),
        "`: this(context, null)` calls the sibling constructor: {:?}",
        reported(&dead, "SudokuCellView")
    );
}
