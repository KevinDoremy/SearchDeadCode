// Reference types - some variants and methods reserved for future use
#![allow(dead_code)]

use super::Location;
use serde::{Deserialize, Serialize};

/// Kind of reference between declarations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ReferenceKind {
    /// Calling a function/method
    Call,

    /// Reading a property/field
    Read,

    /// Writing to a property/field
    Write,

    /// Type reference (in type annotation, generic, etc.)
    Type,

    /// Inheritance (extends/implements)
    Inheritance,

    /// Import statement
    Import,

    /// Instantiation (new/constructor call)
    Instantiation,

    /// Annotation usage
    Annotation,

    /// Cast expression
    Cast,

    /// Generic type argument
    TypeArgument,

    /// Return type
    ReturnType,

    /// Parameter type
    ParameterType,

    /// Override relationship
    Override,

    /// Reflection/class literal reference (e.g., MyClass::class)
    Reflection,

    /// Extension function receiver type
    ExtensionReceiver,

    /// Sealed class subtype
    SealedSubtype,

    /// Property delegation (e.g., by lazy, by Delegates.observable)
    Delegation,

    /// Generic type argument (e.g., List<MyClass>)
    GenericArgument,
}

impl ReferenceKind {
    /// Check if this is a read reference
    pub fn is_read(&self) -> bool {
        matches!(
            self,
            ReferenceKind::Read
                | ReferenceKind::Call
                | ReferenceKind::Type
                | ReferenceKind::TypeArgument
        )
    }

    /// Check if this is a write reference
    pub fn is_write(&self) -> bool {
        matches!(self, ReferenceKind::Write)
    }

    /// Check if this reference counts as "usage" for dead code detection
    pub fn counts_as_usage(&self) -> bool {
        // All references count as usage for now
        true
    }
}

/// A reference from one declaration to another
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reference {
    /// Kind of reference
    pub kind: ReferenceKind,

    /// Location where the reference occurs
    pub location: Location,

    /// The name/identifier used in the reference
    pub name: String,

    /// Whether this is a qualified reference (e.g., com.example.Foo)
    pub is_qualified: bool,
}

impl Reference {
    pub fn new(kind: ReferenceKind, location: Location, name: String) -> Self {
        Self {
            kind,
            location,
            name,
            is_qualified: false,
        }
    }

    pub fn with_qualified(mut self, qualified: bool) -> Self {
        self.is_qualified = qualified;
        self
    }
}

/// Builder for tracking references during parsing
#[derive(Debug, Default)]
pub struct ReferenceCollector {
    /// Collected references
    pub references: Vec<UnresolvedReference>,
}

/// A reference that hasn't been resolved to a specific declaration yet
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnresolvedReference {
    /// The name being referenced
    pub name: String,

    /// Fully qualified name if available
    pub qualified_name: Option<String>,

    /// Kind of reference
    pub kind: ReferenceKind,

    /// Location of the reference
    pub location: Location,

    /// Imports available in scope (for resolution)
    pub imports: Vec<String>,
}

impl ReferenceCollector {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a reference to be resolved later
    pub fn add_reference(
        &mut self,
        name: String,
        kind: ReferenceKind,
        location: Location,
        imports: Vec<String>,
    ) {
        // Check if it's a qualified name
        let (simple_name, qualified_name) = if name.contains('.') {
            let parts: Vec<&str> = name.split('.').collect();
            let last_part = parts
                .last()
                .map(|s| s.to_string())
                .unwrap_or_else(|| name.clone());
            (last_part, Some(name))
        } else {
            (name, None)
        };

        self.references.push(UnresolvedReference {
            name: simple_name,
            qualified_name,
            kind,
            location,
            imports,
        });
    }

    /// Get all collected references
    pub fn drain(&mut self) -> Vec<UnresolvedReference> {
        std::mem::take(&mut self.references)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_reference_kind_is_read() {
        assert!(ReferenceKind::Read.is_read());
        assert!(ReferenceKind::Call.is_read());
        assert!(!ReferenceKind::Write.is_read());
    }

    #[test]
    fn test_reference_kind_is_write() {
        assert!(ReferenceKind::Write.is_write());
        assert!(!ReferenceKind::Read.is_write());
    }

    #[test]
    fn test_reference_collector() {
        let mut collector = ReferenceCollector::new();
        collector.add_reference(
            "MyClass".to_string(),
            ReferenceKind::Type,
            Location::new(PathBuf::from("test.kt"), 1, 1, 0, 10),
            vec!["com.example.MyClass".to_string()],
        );

        assert_eq!(collector.references.len(), 1);
        assert_eq!(collector.references[0].name, "MyClass");
    }

    #[test]
    fn test_qualified_name_parsing() {
        let mut collector = ReferenceCollector::new();
        collector.add_reference(
            "com.example.MyClass".to_string(),
            ReferenceKind::Type,
            Location::new(PathBuf::from("test.kt"), 1, 1, 0, 10),
            vec![],
        );

        assert_eq!(collector.references[0].name, "MyClass");
        assert_eq!(
            collector.references[0].qualified_name,
            Some("com.example.MyClass".to_string())
        );
    }
}
