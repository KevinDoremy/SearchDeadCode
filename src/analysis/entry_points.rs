use crate::config::Config;
use crate::discovery::FileFinder;
use crate::graph::{Declaration, DeclarationId, DeclarationKind, Graph};
use crate::parser::xml::{
    LayoutParser, ManifestParser, MenuParser, NavigationParser, XmlParseResult,
};
use miette::Result;
use std::collections::HashSet;
use std::path::Path;
use tracing::{debug, info};

/// Detects entry points in an Android project
pub struct EntryPointDetector<'a> {
    config: &'a Config,
    manifest_parser: ManifestParser,
    layout_parser: LayoutParser,
    navigation_parser: NavigationParser,
    menu_parser: MenuParser,
}

/// Annotations that mark retention roots (matched by contains)
const ENTRY_ANNOTATIONS: &[&str] = &[
    // Testing
    "Test",
    "Before",
    "After",
    "BeforeEach",
    "AfterEach",
    "BeforeAll",
    "AfterAll",
    "ParameterizedTest",
    "RunWith",
    "Ignore",
    // Compose — Composable deliberately absent: an uncalled composable
    // is dead code like any other function. Previews are IDE-invoked
    // roots and keep whatever they render transitively alive.
    "Preview",
    "PreviewParameter",
    // Dagger/Hilt — Provides/Binds are handled conditionally in
    // is_code_entry_point: a provider is a root only when its
    // produced type is consumed somewhere
    "Inject",
    "BindsInstance",
    "BindsOptionalOf",
    "Module",
    "Component",
    "Subcomponent",
    "HiltAndroidApp",
    "AndroidEntryPoint",
    "HiltViewModel",
    "EntryPoint",
    "InstallIn",
    "Singleton",
    "Reusable",
    "ActivityScoped",
    "FragmentScoped",
    "ViewModelScoped",
    "ServiceScoped",
    // Room Database
    "Dao",
    "Database",
    "Entity",
    "Query",
    "Insert",
    "Update",
    "Delete",
    "RawQuery",
    "Transaction",
    "TypeConverter",
    "TypeConverters",
    "Embedded",
    "Relation",
    "ForeignKey",
    "PrimaryKey",
    "ColumnInfo",
    // Retrofit
    "GET",
    "POST",
    "PUT",
    "DELETE",
    "PATCH",
    "HEAD",
    "OPTIONS",
    "HTTP",
    "Path",
    "Body",
    "Field",
    "FieldMap",
    "Header",
    "HeaderMap",
    "Headers",
    "Multipart",
    "FormUrlEncoded",
    "Streaming",
    "Url",
    // Serialization
    "Serializable",
    "Parcelize",
    "JsonClass",
    "Json",
    "JsonAdapter",
    "SerializedName",
    "Expose",
    "SerialName",
    "Contextual",
    "Polymorphic",
    // Android specific
    "BindingAdapter",
    "InverseBindingAdapter",
    "BindingMethod",
    "BindingMethods",
    "BindingConversion",
    // Jvm* interop annotations deliberately absent: @JvmOverloads,
    // @JvmStatic, @JvmField, @JvmName change how a symbol is exposed
    // to Java, they do not make anything reachable by themselves
    // Reflection markers
    "Keep",
    "KeepPublicApi",
    // WebView JS bridge: invoked reflectively from page JavaScript
    "JavascriptInterface",
    // WorkManager
    "HiltWorker",
    // Lifecycle
    "OnLifecycleEvent",
    // Navigation
    "NavGraph",
    "NavDestination",
    // Event Bus
    "Subscribe",
    // Coroutines/Flow
    "FlowPreview",
    "ExperimentalCoroutinesApi",
    // Kotlin Multiplatform
    "JsExport",
    "JsName",
    // Native
    "CName",
    // Koin
    "KoinViewModel",
    "Factory",
    "Single",
];

impl<'a> EntryPointDetector<'a> {
    pub fn new(config: &'a Config) -> Self {
        Self {
            config,
            manifest_parser: ManifestParser::new(),
            layout_parser: LayoutParser::new(),
            navigation_parser: NavigationParser::new(),
            menu_parser: MenuParser::new(),
        }
    }

    /// Detect all entry points in the project
    pub fn detect(&self, graph: &Graph, root: &Path) -> Result<HashSet<DeclarationId>> {
        self.detect_with_keep_rules(graph, root, true)
    }

    /// Same detection with the ProGuard/R8 keep category switchable off.
    /// A -keep rule instructs the shrinker to retain BYTES for reflection;
    /// it is not evidence the SOURCE is alive — the island analysis roots
    /// only keeps that spell an exact name, and labels the rest.
    pub fn detect_with_keep_rules(
        &self,
        graph: &Graph,
        root: &Path,
        honor_keep_rules: bool,
    ) -> Result<HashSet<DeclarationId>> {
        let mut entry_points = HashSet::new();

        // 1. Detect entry points from code analysis
        self.detect_code_entry_points(graph, &mut entry_points);

        // 2. Detect entry points from AndroidManifest.xml
        if self.config.android.parse_manifest {
            self.detect_manifest_entry_points(graph, root, &mut entry_points)?;
        }

        // 3. Detect entry points from layout XMLs
        if self.config.android.parse_layouts {
            self.detect_layout_entry_points(graph, root, &mut entry_points)?;
        }

        // 4. Detect entry points from navigation XMLs
        self.detect_navigation_entry_points(graph, root, &mut entry_points)?;

        // 5. Detect entry points from menu XMLs
        self.detect_menu_entry_points(graph, root, &mut entry_points)?;

        // 6. Add explicitly configured entry points
        self.add_configured_entry_points(graph, &mut entry_points);

        // 6b. Respect ProGuard/R8 -keep rules: kept classes are retained
        let keep_patterns = if honor_keep_rules {
            crate::analysis::keep_rules::collect_keep_patterns(root)
        } else {
            Vec::new()
        };
        if !keep_patterns.is_empty() {
            for decl in graph.declarations() {
                if let Some(fqn) = &decl.fully_qualified_name {
                    if keep_patterns.iter().any(|p| p.matches(fqn)) {
                        entry_points.insert(decl.id.clone());
                    }
                }
            }
        }

        // 7. Apply retain patterns
        self.apply_retain_patterns(graph, &mut entry_points);

        // 8. Generated-code naming conventions: a reference to
        // PriceCatalog_Factory or CheckoutStepDirections lives in build/
        // (never parsed), but proves the source class is alive
        self.detect_generated_convention_roots(graph, root, &mut entry_points);

        // 9. res/xml roots: preference screens and shortcuts reference
        // classes by FQN (custom tags, targetClass, app:fragment)
        self.detect_res_xml_roots(graph, root, &mut entry_points);

        // 10. Lombok fields: @Getter/@Setter/@Data generate accessors
        // the graph cannot see — a generated-accessor call in the
        // corpus keeps the field alive
        self.detect_lombok_accessor_roots(graph, root, &mut entry_points);

        info!("Detected {} entry points", entry_points.len());

        Ok(entry_points)
    }

    /// Detect entry points from code analysis (annotations, inheritance)
    fn detect_code_entry_points(&self, graph: &Graph, entry_points: &mut HashSet<DeclarationId>) {
        for decl in graph.declarations() {
            if self.is_code_entry_point(graph, decl) {
                debug!(
                    "Code entry point: {} ({})",
                    decl.name,
                    decl.kind.display_name()
                );
                entry_points.insert(decl.id.clone());
            }
        }
    }

    /// Check if a declaration is an entry point based on code analysis
    fn is_code_entry_point(&self, graph: &Graph, decl: &Declaration) -> bool {
        // DI providers (@Provides/@Binds) are roots only when their produced
        // type is actually consumed — an orphan binding is dead code
        let is_di_provider = decl
            .annotations
            .iter()
            .any(|a| a.contains("Provides") || a.contains("Binds"));
        if is_di_provider && di_binding_is_consumed(graph, decl) {
            return true;
        }

        // Check Android components by inheritance
        if decl.is_android_entry_point() {
            return true;
        }

        // Check annotations
        for annotation in &decl.annotations {
            if self.is_entry_point_annotation(annotation) {
                return true;
            }
        }

        // Check for main functions — Kotlin top-level, or Java's
        // public static void main (a Method, not a Function)
        if decl.name == "main"
            && (decl.kind == DeclarationKind::Function
                || (decl.kind == DeclarationKind::Method && decl.is_static))
        {
            return true;
        }

        // Check for serialization
        if decl.annotations.iter().any(|a| {
            a.contains("Serializable")
                || a.contains("Parcelize")
                || a.contains("Entity")
                || a.contains("JsonClass")
        }) {
            return true;
        }

        false
    }

    /// Check if an annotation marks an entry point
    fn is_entry_point_annotation(&self, annotation: &str) -> bool {
        ENTRY_ANNOTATIONS.iter().any(|e| annotation.contains(e))
    }

    /// L'annotation d'entrée qui retient cette déclaration, en la cherchant
    /// sur la déclaration elle-même puis sur ses membres (un @Inject sur le
    /// constructeur retient la classe). Pour que --why-alive nomme la cause
    /// au lieu d'un « it is an entry point » opaque.
    pub fn entry_annotation_reason(
        graph: &crate::graph::Graph,
        decl: &crate::graph::Declaration,
    ) -> Option<String> {
        let find = |d: &crate::graph::Declaration| -> Option<String> {
            d.annotations
                .iter()
                .find(|a| ENTRY_ANNOTATIONS.iter().any(|e| a.contains(e)))
                .cloned()
        };
        if let Some(a) = find(decl) {
            return Some(a);
        }
        for child_id in graph.get_children(&decl.id) {
            if let Some(child) = graph.get_declaration(child_id) {
                if let Some(a) = find(child) {
                    let site = match child.kind {
                        crate::graph::DeclarationKind::Constructor => "constructor",
                        _ => child.name.as_str(),
                    };
                    return Some(format!("{a} on its {site}"));
                }
            }
        }
        None
    }

    /// How many declarations each retention annotation keeps alive —
    /// with the same `contains` matching the detector itself uses, so
    /// the audit reflects reality, not an idealized exact-match world.
    pub fn annotation_retention_counts(&self, graph: &crate::graph::Graph) -> Vec<(String, usize)> {
        use std::collections::HashMap;
        let mut counts: HashMap<&str, usize> = HashMap::new();
        for decl in graph.declarations() {
            for annotation in &decl.annotations {
                for entry in ENTRY_ANNOTATIONS {
                    if annotation.contains(entry) {
                        *counts.entry(entry).or_default() += 1;
                    }
                }
            }
        }
        let mut ranked: Vec<(String, usize)> = counts
            .into_iter()
            .map(|(name, count)| (name.to_string(), count))
            .collect();
        ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        ranked
    }

    /// Detect entry points from AndroidManifest.xml
    fn detect_manifest_entry_points(
        &self,
        graph: &Graph,
        root: &Path,
        entry_points: &mut HashSet<DeclarationId>,
    ) -> Result<()> {
        let finder = FileFinder::new(self.config);
        let manifests = finder.find_manifests(root)?;

        for manifest in manifests {
            let contents = manifest.read_contents()?;
            let result = self.manifest_parser.parse(&manifest.path, &contents)?;

            self.add_xml_references(graph, &result, entry_points);
        }

        Ok(())
    }

    /// Detect entry points from layout XMLs
    fn detect_layout_entry_points(
        &self,
        graph: &Graph,
        root: &Path,
        entry_points: &mut HashSet<DeclarationId>,
    ) -> Result<()> {
        let finder = FileFinder::new(self.config);
        let layouts = finder.find_layouts(root)?;

        let mut total_binding_vars = 0;
        let mut total_method_refs = 0;

        for layout in &layouts {
            let contents = layout.read_contents()?;
            let result = self.layout_parser.parse(&layout.path, &contents)?;

            total_binding_vars += result.binding_variables.len();
            total_method_refs += result.method_references.len();

            self.add_xml_references(graph, &result, entry_points);
        }

        if total_method_refs > 0 {
            info!(
                "Parsed {} layout files: {} binding variables, {} method references",
                layouts.len(),
                total_binding_vars,
                total_method_refs
            );
        }

        Ok(())
    }

    /// Detect entry points from navigation XMLs (fragments, dialogs, activities)
    fn detect_navigation_entry_points(
        &self,
        graph: &Graph,
        root: &Path,
        entry_points: &mut HashSet<DeclarationId>,
    ) -> Result<()> {
        let finder = FileFinder::new(self.config);
        let navigation_files = finder.find_navigation(root)?;

        if !navigation_files.is_empty() {
            debug!("Found {} navigation XML files", navigation_files.len());
        }

        for nav_file in navigation_files {
            let contents = nav_file.read_contents()?;
            let result = self.navigation_parser.parse(&nav_file.path, &contents)?;

            self.add_xml_references(graph, &result, entry_points);
        }

        Ok(())
    }

    /// Detect entry points from menu XMLs (action view classes, action providers)
    fn detect_menu_entry_points(
        &self,
        graph: &Graph,
        root: &Path,
        entry_points: &mut HashSet<DeclarationId>,
    ) -> Result<()> {
        let finder = FileFinder::new(self.config);
        let menu_files = finder.find_menus(root)?;

        if !menu_files.is_empty() {
            debug!("Found {} menu XML files", menu_files.len());
        }

        for menu_file in menu_files {
            let contents = menu_file.read_contents()?;
            let result = self.menu_parser.parse(&menu_file.path, &contents)?;

            self.add_xml_references(graph, &result, entry_points);
        }

        Ok(())
    }

    /// Add entry points from XML parse results
    fn add_xml_references(
        &self,
        graph: &Graph,
        result: &XmlParseResult,
        entry_points: &mut HashSet<DeclarationId>,
    ) {
        // Handle class references
        for class_ref in &result.class_references {
            // Try to find by fully qualified name
            // Tous les porteurs du FQN : deux source sets (main, debug)
            // déclarent la même classe, et n'en raciner qu'un rapportait
            // l'autre mort alors que le manifeste nomme les deux.
            let carriers = graph.find_all_by_fqn(class_ref);
            if !carriers.is_empty() {
                for decl in carriers {
                    debug!("XML entry point: {} (fqn)", decl.name);
                    entry_points.insert(decl.id.clone());
                }
                continue;
            }

            // Try to find by simple name (last component)
            let simple_name = class_ref.split('.').next_back().unwrap_or(class_ref);
            let candidates = graph.find_by_name(simple_name);
            for candidate in candidates {
                debug!("XML entry point: {} (simple)", candidate.name);
                entry_points.insert(candidate.id.clone());
            }
        }

        // Handle method references from data binding
        if !result.method_references.is_empty() {
            debug!(
                "Processing {} method references from data binding",
                result.method_references.len()
            );
        }
        for method_ref in &result.method_references {
            debug!(
                "Data binding method ref: {}.{}",
                method_ref.class_fqn, method_ref.method_name
            );
            self.add_method_reference(
                graph,
                &method_ref.class_fqn,
                &method_ref.method_name,
                entry_points,
            );
        }
    }

    /// Add a method reference as an entry point
    fn add_method_reference(
        &self,
        graph: &Graph,
        class_fqn: &str,
        method_name: &str,
        entry_points: &mut HashSet<DeclarationId>,
    ) {
        // Find the class first
        let class_decl = if let Some(decl) = graph.find_by_fqn(class_fqn) {
            Some(decl)
        } else {
            // Try by simple name
            let simple_name = class_fqn.split('.').next_back().unwrap_or(class_fqn);
            graph.find_by_name(simple_name).into_iter().next()
        };

        if let Some(class) = class_decl {
            // Find the method as a child of this class
            let children = graph.get_children(&class.id);
            for child in &children {
                if let Some(child_decl) = graph.get_declaration(child) {
                    if child_decl.name == method_name {
                        debug!(
                            "Data binding entry point: {}.{} (method)",
                            class.name, method_name
                        );
                        entry_points.insert((*child).clone());
                        return;
                    }
                }
            }

            // Also search by name in case of inheritance or extension functions
            let method_candidates = graph.find_by_name(method_name);
            for candidate in method_candidates {
                // Check if this method's parent matches the class
                if let Some(parent_id) = &candidate.parent {
                    if parent_id == &class.id {
                        debug!(
                            "Data binding entry point: {}.{} (by parent)",
                            class.name, method_name
                        );
                        entry_points.insert(candidate.id.clone());
                        return;
                    }
                }
            }

            // Log at info level if we couldn't find the method
            // Only log for methods that look like view model callbacks
            if method_name.starts_with("on") && method_name.len() > 3 {
                info!(
                    "Data binding: could not find method {} in class {} (children: {})",
                    method_name,
                    class_fqn,
                    children.len()
                );
            }
        } else {
            // Log at info level if we couldn't find the class
            info!(
                "Data binding: could not find class {} for method {}",
                class_fqn, method_name
            );
        }
    }

    /// Add explicitly configured entry points
    fn add_configured_entry_points(
        &self,
        graph: &Graph,
        entry_points: &mut HashSet<DeclarationId>,
    ) {
        for entry_point in &self.config.entry_points {
            let carriers = graph.find_all_by_fqn(entry_point);
            if !carriers.is_empty() {
                for decl in carriers {
                    debug!("Configured entry point: {}", decl.name);
                    entry_points.insert(decl.id.clone());
                }
            } else {
                // Try as simple name
                for decl in graph.find_by_name(entry_point) {
                    debug!("Configured entry point (by name): {}", decl.name);
                    entry_points.insert(decl.id.clone());
                }
            }
        }
    }

    /// Apply retain patterns to mark additional entry points
    fn apply_retain_patterns(&self, graph: &Graph, entry_points: &mut HashSet<DeclarationId>) {
        for decl in graph.declarations() {
            // Check config retain patterns
            for pattern in &self.config.retain_patterns {
                if decl.matches_pattern(pattern) {
                    debug!("Retained by pattern '{}': {}", pattern, decl.name);
                    entry_points.insert(decl.id.clone());
                }
            }

            // Check Android component patterns
            if self.config.android.auto_retain_components {
                for pattern in &self.config.android.component_patterns {
                    if decl.matches_pattern(pattern) {
                        debug!("Retained by component pattern '{}': {}", pattern, decl.name);
                        entry_points.insert(decl.id.clone());
                    }
                }
            }
        }
    }

    /// Map references to generated classes back to their source: a call
    /// to PriceCatalog_Factory, OrderStore_Impl or CheckoutStepDirections
    /// resolves to nothing (build/ is never parsed) yet proves the base
    /// class is alive. Only kicks in when the generated name has no
    /// declaration of its own — a real class named DaggerTool says
    /// nothing about a class named Tool.
    fn detect_generated_convention_roots(
        &self,
        graph: &Graph,
        root: &Path,
        entry_points: &mut HashSet<DeclarationId>,
    ) {
        use regex::Regex;
        use std::sync::LazyLock;
        static GENERATED_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
            vec![
                Regex::new(r"\bDagger([A-Z]\w*)\b").unwrap(),
                Regex::new(r"\b([A-Z]\w*?)_(?:Factory|MembersInjector|Impl)\b").unwrap(),
                Regex::new(r"\b([A-Z]\w*?)(?:Directions|Args)\b").unwrap(),
            ]
        });

        let mut bases: HashSet<String> = HashSet::new();
        for entry in walkdir::WalkDir::new(root)
            .into_iter()
            .filter_entry(|e| {
                if e.depth() == 0 {
                    return true;
                }
                let name = e.file_name().to_string_lossy();
                !(name.starts_with('.') || name == "build" || name == "node_modules")
            })
            .filter_map(Result::ok)
            .filter(|e| {
                let name = e.file_name().to_string_lossy();
                e.file_type().is_file() && (name.ends_with(".kt") || name.ends_with(".java"))
            })
        {
            let Ok(content) = std::fs::read_to_string(entry.path()) else {
                continue;
            };
            for pattern in GENERATED_PATTERNS.iter() {
                for cap in pattern.captures_iter(&content) {
                    let generated_name = cap.get(0).unwrap().as_str();
                    if !graph.find_by_name(generated_name).is_empty() {
                        continue;
                    }
                    bases.insert(cap[1].to_string());
                }
            }
        }

        for base in bases {
            for decl in graph.find_by_name(&base) {
                debug!("Generated-convention root: {}", decl.name);
                entry_points.insert(decl.id.clone());
            }
        }
    }

    /// Classes referenced by FQN from res/xml files: custom preference
    /// tags, shortcut targetClass, preference app:fragment. The graph
    /// cannot see XML, so these references live nowhere else.
    fn detect_res_xml_roots(
        &self,
        graph: &Graph,
        root: &Path,
        entry_points: &mut HashSet<DeclarationId>,
    ) {
        use regex::Regex;
        use std::sync::LazyLock;
        // custom tags are FQNs with an uppercase last segment; attribute
        // values go through the same FQN shape
        static FQN_TAG_RE: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r"<([a-z][\w.]*\.[A-Z]\w*)[\s/>]").unwrap());
        static CLASS_ATTR_RE: LazyLock<Regex> = LazyLock::new(|| {
            Regex::new(
                r#"(?:android:targetClass|app:fragment|android:name|class)\s*=\s*"([\w.]+)""#,
            )
            .unwrap()
        });

        let mut fqns: HashSet<String> = HashSet::new();
        for entry in walkdir::WalkDir::new(root)
            .into_iter()
            .filter_entry(|e| {
                if e.depth() == 0 {
                    return true;
                }
                let name = e.file_name().to_string_lossy();
                !(name.starts_with('.') || name == "build" || name == "node_modules")
            })
            .filter_map(Result::ok)
            .filter(|e| {
                e.file_type().is_file()
                    && e.file_name().to_string_lossy().ends_with(".xml")
                    && e.path()
                        .parent()
                        .and_then(|p| p.file_name())
                        .map(|n| n == "xml")
                        .unwrap_or(false)
            })
        {
            let Ok(content) = std::fs::read_to_string(entry.path()) else {
                continue;
            };
            for cap in FQN_TAG_RE.captures_iter(&content) {
                fqns.insert(cap[1].to_string());
            }
            for cap in CLASS_ATTR_RE.captures_iter(&content) {
                if cap[1].contains('.') {
                    fqns.insert(cap[1].to_string());
                }
            }
        }

        for fqn in fqns {
            let simple = fqn.rsplit('.').next().unwrap_or(&fqn);
            for decl in graph.find_by_name(simple) {
                let fqn_matches = decl
                    .fully_qualified_name
                    .as_deref()
                    .map(|declared| declared == fqn)
                    .unwrap_or(true); // no FQN recorded: simple-name match is enough
                if fqn_matches {
                    debug!("res/xml root: {}", decl.name);
                    entry_points.insert(decl.id.clone());
                }
            }
        }
    }

    /// Fields of Lombok-annotated classes reached through generated
    /// accessors: getX()/setX()/isX()/builder().x() calls resolve to
    /// nothing (the methods only exist post-annotation-processing) but
    /// prove the field is alive. A Lombok field nobody touches through
    /// any accessor stays reportable.
    fn detect_lombok_accessor_roots(
        &self,
        graph: &Graph,
        root: &Path,
        entry_points: &mut HashSet<DeclarationId>,
    ) {
        const LOMBOK_CLASS_ANNOTATIONS: &[&str] = &["Getter", "Setter", "Data", "Value", "Builder"];
        let lombok_fields: Vec<&Declaration> = graph
            .declarations()
            .filter(|decl| decl.kind == DeclarationKind::Field)
            .filter(|decl| {
                decl.parent
                    .as_ref()
                    .and_then(|parent| graph.get_declaration(parent))
                    .map(|class| {
                        class
                            .annotations
                            .iter()
                            .any(|a| LOMBOK_CLASS_ANNOTATIONS.iter().any(|l| a.contains(l)))
                    })
                    .unwrap_or(false)
            })
            .collect();
        if lombok_fields.is_empty() {
            return;
        }

        let mut corpus = String::new();
        for entry in walkdir::WalkDir::new(root)
            .into_iter()
            .filter_entry(|e| {
                if e.depth() == 0 {
                    return true;
                }
                let name = e.file_name().to_string_lossy();
                !(name.starts_with('.') || name == "build" || name == "node_modules")
            })
            .filter_map(Result::ok)
            .filter(|e| {
                let name = e.file_name().to_string_lossy();
                e.file_type().is_file() && (name.ends_with(".kt") || name.ends_with(".java"))
            })
        {
            if let Ok(content) = std::fs::read_to_string(entry.path()) {
                corpus.push_str(&content);
                corpus.push('\n');
            }
        }

        for field in lombok_fields {
            let mut chars = field.name.chars();
            let capitalized = match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => continue,
            };
            let accessor_called = corpus.contains(&format!("get{capitalized}("))
                || corpus.contains(&format!("set{capitalized}("))
                || corpus.contains(&format!("is{capitalized}("))
                || corpus.contains(&format!(".{}(", field.name));
            if accessor_called {
                debug!("Lombok accessor root: {}", field.name);
                entry_points.insert(field.id.clone());
            }
        }
    }
}

/// Is the type produced by this @Provides/@Binds method consumed anywhere?
///
/// Consumption = an incoming reference to the produced type that is neither
/// the provider itself, nor another provider returning the same type, nor a
/// subtype implementing it (an interface's implementations are not users).
/// Providers without return-type information stay conservative roots.
pub(crate) fn di_binding_is_consumed(graph: &Graph, provider: &Declaration) -> bool {
    let Some(produced) = provider.type_name.as_deref() else {
        return true;
    };
    let produced_simple = produced
        .split('<')
        .next()
        .unwrap_or(produced)
        .trim()
        .trim_end_matches('?');
    if produced_simple.is_empty() {
        return true;
    }

    // Type produit introuvable dans le projet = type externe (lib) :
    // la consommation passe par des sites d'injection que le graphe ne
    // relie pas à un nœud local — indécidable, donc bénéfice du doute.
    // Sans ça, tous les providers de dispatchers/players/prefs des libs
    // sortaient « never used ».
    let targets = graph.find_by_name(produced_simple);
    if targets.is_empty() {
        return true;
    }

    for target in targets {
        for (referencer, reference) in graph.get_references_to(&target.id) {
            if referencer.id == provider.id {
                continue;
            }
            let is_provider_of_same = referencer
                .annotations
                .iter()
                .any(|a| a.contains("Provides") || a.contains("Binds"))
                && referencer
                    .type_name
                    .as_deref()
                    .map(|t| t.split('<').next().unwrap_or(t).trim() == produced_simple)
                    .unwrap_or(false);
            if is_provider_of_same {
                continue;
            }
            // Implémenter l'interface n'est pas la consommer — mais on
            // skippe la RÉFÉRENCE d'héritage, pas le référenceur entier :
            // `class Delegate(inner: I) : I by inner` implémente ET
            // consomme, sa référence de paramètre doit compter.
            if reference.kind == crate::graph::ReferenceKind::Inheritance {
                continue;
            }
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_entry_point_annotation() {
        let config = Config::default();
        let detector = EntryPointDetector::new(&config);

        assert!(detector.is_entry_point_annotation("@Test"));
        assert!(detector.is_entry_point_annotation("@Preview"));
        assert!(detector.is_entry_point_annotation("@HiltViewModel"));
        assert!(!detector.is_entry_point_annotation("@Override"));
        // an uncalled composable is dead code — no blanket retention
        assert!(!detector.is_entry_point_annotation("@Composable"));
    }
}
