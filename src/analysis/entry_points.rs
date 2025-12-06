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

        // 7. Apply retain patterns
        self.apply_retain_patterns(graph, &mut entry_points);

        info!("Detected {} entry points", entry_points.len());

        Ok(entry_points)
    }

    /// Detect entry points from code analysis (annotations, inheritance)
    fn detect_code_entry_points(&self, graph: &Graph, entry_points: &mut HashSet<DeclarationId>) {
        for decl in graph.declarations() {
            if self.is_code_entry_point(decl) {
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
    fn is_code_entry_point(&self, decl: &Declaration) -> bool {
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

        // Check for main functions
        if decl.kind == DeclarationKind::Function && decl.name == "main" {
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
        let entry_annotations = [
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
            // Compose
            "Composable",
            "Preview",
            "PreviewParameter",
            // Dagger/Hilt
            "Inject",
            "Provides",
            "Binds",
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
            "JvmStatic",
            "JvmOverloads",
            "JvmField",
            "JvmName",
            // Reflection markers
            "Keep",
            "KeepPublicApi",
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

        for entry in &entry_annotations {
            if annotation.contains(entry) {
                return true;
            }
        }

        false
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

        for layout in layouts {
            let contents = layout.read_contents()?;
            let result = self.layout_parser.parse(&layout.path, &contents)?;

            self.add_xml_references(graph, &result, entry_points);
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
        for class_ref in &result.class_references {
            // Try to find by fully qualified name
            if let Some(decl) = graph.find_by_fqn(class_ref) {
                debug!("XML entry point: {} (fqn)", decl.name);
                entry_points.insert(decl.id.clone());
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
    }

    /// Add explicitly configured entry points
    fn add_configured_entry_points(
        &self,
        graph: &Graph,
        entry_points: &mut HashSet<DeclarationId>,
    ) {
        for entry_point in &self.config.entry_points {
            if let Some(decl) = graph.find_by_fqn(entry_point) {
                debug!("Configured entry point: {}", decl.name);
                entry_points.insert(decl.id.clone());
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_entry_point_annotation() {
        let config = Config::default();
        let detector = EntryPointDetector::new(&config);

        assert!(detector.is_entry_point_annotation("@Test"));
        assert!(detector.is_entry_point_annotation("@Composable"));
        assert!(detector.is_entry_point_annotation("@HiltViewModel"));
        assert!(!detector.is_entry_point_annotation("@Override"));
    }
}
