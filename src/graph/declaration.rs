// Declaration types - some fields and methods reserved for future use
#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Unique identifier for a declaration
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DeclarationId {
    /// File path
    pub file: PathBuf,
    /// Starting byte offset in file
    pub start: usize,
    /// Ending byte offset in file
    pub end: usize,
}

impl DeclarationId {
    pub fn new(file: PathBuf, start: usize, end: usize) -> Self {
        Self { file, start, end }
    }
}

impl std::fmt::Display for DeclarationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}:{}", self.file.display(), self.start, self.end)
    }
}

/// Kind of declaration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DeclarationKind {
    // Classes and types
    Class,
    Interface,
    Object, // Kotlin object
    Enum,
    EnumCase,
    TypeAlias,
    Annotation,

    // Functions
    Function,
    Method,
    Constructor,
    Property, // Kotlin property
    Field,    // Java field
    Parameter,

    // Imports
    Import,

    // Other
    Package,
    File,
}

impl DeclarationKind {
    pub fn is_type(&self) -> bool {
        matches!(
            self,
            DeclarationKind::Class
                | DeclarationKind::Interface
                | DeclarationKind::Object
                | DeclarationKind::Enum
                | DeclarationKind::TypeAlias
                | DeclarationKind::Annotation
        )
    }

    pub fn is_callable(&self) -> bool {
        matches!(
            self,
            DeclarationKind::Function | DeclarationKind::Method | DeclarationKind::Constructor
        )
    }

    pub fn is_member(&self) -> bool {
        matches!(
            self,
            DeclarationKind::Method
                | DeclarationKind::Property
                | DeclarationKind::Field
                | DeclarationKind::Constructor
        )
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            DeclarationKind::Class => "class",
            DeclarationKind::Interface => "interface",
            DeclarationKind::Object => "object",
            DeclarationKind::Enum => "enum",
            DeclarationKind::EnumCase => "enum case",
            DeclarationKind::TypeAlias => "type alias",
            DeclarationKind::Annotation => "annotation",
            DeclarationKind::Function => "function",
            DeclarationKind::Method => "method",
            DeclarationKind::Constructor => "constructor",
            DeclarationKind::Property => "property",
            DeclarationKind::Field => "field",
            DeclarationKind::Parameter => "parameter",
            DeclarationKind::Import => "import",
            DeclarationKind::Package => "package",
            DeclarationKind::File => "file",
        }
    }
}

/// Visibility modifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum Visibility {
    #[default]
    Public,
    Private,
    Protected,
    Internal,       // Kotlin internal
    PackagePrivate, // Java default
}

impl Visibility {
    pub fn from_kotlin_modifier(modifier: &str) -> Self {
        match modifier {
            "public" => Visibility::Public,
            "private" => Visibility::Private,
            "protected" => Visibility::Protected,
            "internal" => Visibility::Internal,
            _ => Visibility::Public, // Kotlin default is public
        }
    }

    pub fn from_java_modifiers(modifiers: &[&str]) -> Self {
        if modifiers.contains(&"private") {
            Visibility::Private
        } else if modifiers.contains(&"protected") {
            Visibility::Protected
        } else if modifiers.contains(&"public") {
            Visibility::Public
        } else {
            Visibility::PackagePrivate // Java default
        }
    }
}

/// Location in source code
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Location {
    /// File path
    pub file: PathBuf,
    /// Line number (1-indexed)
    pub line: usize,
    /// Column number (1-indexed)
    pub column: usize,
    /// Starting byte offset
    pub start_byte: usize,
    /// Ending byte offset
    pub end_byte: usize,
}

impl Location {
    pub fn new(
        file: PathBuf,
        line: usize,
        column: usize,
        start_byte: usize,
        end_byte: usize,
    ) -> Self {
        Self {
            file,
            line,
            column,
            start_byte,
            end_byte,
        }
    }
}

impl std::fmt::Display for Location {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}:{}", self.file.display(), self.line, self.column)
    }
}

/// A declaration in the source code
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Declaration {
    /// Unique identifier
    pub id: DeclarationId,

    /// Simple name (e.g., "MainActivity")
    pub name: String,

    /// Fully qualified name (e.g., "com.example.app.MainActivity")
    pub fully_qualified_name: Option<String>,

    /// Kind of declaration
    pub kind: DeclarationKind,

    /// Visibility modifier
    pub visibility: Visibility,

    /// Location in source code
    pub location: Location,

    /// Parent declaration (e.g., class for a method)
    pub parent: Option<DeclarationId>,

    /// Whether this is a static member
    pub is_static: bool,

    /// Whether this is an abstract member
    pub is_abstract: bool,

    /// Annotations on this declaration
    pub annotations: Vec<String>,

    /// Extended/implemented types (for classes)
    pub super_types: Vec<String>,

    /// Modifiers (for additional analysis)
    pub modifiers: Vec<String>,

    /// Language (Kotlin or Java)
    pub language: Language,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    Kotlin,
    Java,
}

impl Declaration {
    pub fn new(
        id: DeclarationId,
        name: String,
        kind: DeclarationKind,
        location: Location,
        language: Language,
    ) -> Self {
        Self {
            id,
            name,
            fully_qualified_name: None,
            kind,
            visibility: Visibility::default(),
            location,
            parent: None,
            is_static: false,
            is_abstract: false,
            annotations: Vec::new(),
            super_types: Vec::new(),
            modifiers: Vec::new(),
            language,
        }
    }

    /// Check if this declaration is an Android entry point
    pub fn is_android_entry_point(&self) -> bool {
        // Check super types for Android components
        let android_components = [
            "Activity",
            "AppCompatActivity",
            "FragmentActivity",
            "ComponentActivity",
            "Fragment",
            "DialogFragment",
            "Service",
            "IntentService",
            "BroadcastReceiver",
            "ContentProvider",
            "Application",
            "ViewModel",
            "AndroidViewModel",
        ];

        for super_type in &self.super_types {
            for component in &android_components {
                if super_type.contains(component) {
                    return true;
                }
            }
        }

        // Check annotations
        let entry_annotations = [
            "Composable",
            "Test",
            "Before",
            "After",
            "BeforeEach",
            "AfterEach",
            "JvmStatic",
            "BindingAdapter",
            "Provides",
            "Binds",
            "Inject",
            "HiltAndroidApp",
            "AndroidEntryPoint",
            "HiltViewModel",
        ];

        for annotation in &self.annotations {
            for entry_ann in &entry_annotations {
                if annotation.contains(entry_ann) {
                    return true;
                }
            }
        }

        // Check for main function
        if self.kind == DeclarationKind::Function && self.name == "main" {
            return true;
        }

        false
    }

    /// Check if this declaration should be retained based on patterns
    pub fn matches_pattern(&self, pattern: &str) -> bool {
        // Simple wildcard matching
        if let Some(suffix) = pattern.strip_prefix('*') {
            self.name.ends_with(suffix)
        } else if let Some(prefix) = pattern.strip_suffix('*') {
            self.name.starts_with(prefix)
        } else {
            self.name == pattern
                || self
                    .fully_qualified_name
                    .as_ref()
                    .map(|fqn| fqn == pattern)
                    .unwrap_or(false)
        }
    }

    /// Get a display string for this declaration
    pub fn display(&self) -> String {
        format!(
            "{} {} ({})",
            self.kind.display_name(),
            self.name,
            self.location
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_declaration_kind_display() {
        assert_eq!(DeclarationKind::Class.display_name(), "class");
        assert_eq!(DeclarationKind::Function.display_name(), "function");
    }

    #[test]
    fn test_visibility_from_kotlin() {
        assert_eq!(
            Visibility::from_kotlin_modifier("private"),
            Visibility::Private
        );
        assert_eq!(
            Visibility::from_kotlin_modifier("internal"),
            Visibility::Internal
        );
    }

    #[test]
    fn test_visibility_from_java() {
        assert_eq!(
            Visibility::from_java_modifiers(&["private"]),
            Visibility::Private
        );
        assert_eq!(
            Visibility::from_java_modifiers(&[]),
            Visibility::PackagePrivate
        );
    }

    #[test]
    fn test_is_android_entry_point() {
        let mut decl = Declaration::new(
            DeclarationId::new(PathBuf::from("test.kt"), 0, 100),
            "MainActivity".to_string(),
            DeclarationKind::Class,
            Location::new(PathBuf::from("test.kt"), 1, 1, 0, 100),
            Language::Kotlin,
        );

        assert!(!decl.is_android_entry_point());

        decl.super_types.push("AppCompatActivity".to_string());
        assert!(decl.is_android_entry_point());
    }

    #[test]
    fn test_matches_pattern() {
        let decl = Declaration::new(
            DeclarationId::new(PathBuf::from("test.kt"), 0, 100),
            "MainActivity".to_string(),
            DeclarationKind::Class,
            Location::new(PathBuf::from("test.kt"), 1, 1, 0, 100),
            Language::Kotlin,
        );

        assert!(decl.matches_pattern("*Activity"));
        assert!(decl.matches_pattern("Main*"));
        assert!(decl.matches_pattern("MainActivity"));
        assert!(!decl.matches_pattern("*Fragment"));
    }
}
