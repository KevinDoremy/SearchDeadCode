//! Single Implementation Interface Detector
//!
//! Detects interfaces that have only one implementation in the codebase.
//! This is often an unnecessary abstraction that adds complexity.
//!
//! ## Anti-Pattern
//!
//! ```kotlin
//! interface UserRepository {
//!     fun getUsers(): List<User>
//! }
//!
//! class UserRepositoryImpl : UserRepository {  // Only implementation!
//!     override fun getUsers() = listOf()
//! }
//! ```
//!
//! ## Why It's Bad
//!
//! - Extra boilerplate (interface + impl class)
//! - Navigation in IDE is harder (jump to interface, then to impl)
//! - Premature abstraction ("I might need another impl someday")
//! - Violates YAGNI (You Aren't Gonna Need It)
//!
//! ## Exceptions (When Interface IS Needed)
//!
//! - Testing: interface allows fake/mock implementations
//! - Platform abstraction: Android vs iOS implementations
//! - Plugin architecture: multiple implementations by design
//!
//! ## Better Approach
//!
//! - Use class directly when single implementation
//! - Extract interface when second implementation is needed

use super::Detector;
use crate::analysis::{Confidence, DeadCode, DeadCodeIssue};
use crate::graph::{DeclarationKind, Graph};
use std::collections::HashMap;

/// Detector for interfaces with only one implementation
pub struct SingleImplInterfaceDetector {
    /// Skip interfaces commonly needed for testing
    skip_test_interfaces: bool,
    /// Skip repository/datasource interfaces (often justified)
    skip_repository_interfaces: bool,
}

impl SingleImplInterfaceDetector {
    pub fn new() -> Self {
        Self {
            skip_test_interfaces: true,
            skip_repository_interfaces: false,
        }
    }

    /// Don't skip any interfaces
    #[allow(dead_code)]
    pub fn strict(mut self) -> Self {
        self.skip_test_interfaces = false;
        self.skip_repository_interfaces = false;
        self
    }

    /// Also skip repository/datasource interfaces
    #[allow(dead_code)]
    pub fn skip_repositories(mut self) -> Self {
        self.skip_repository_interfaces = true;
        self
    }

    /// Check if interface should be skipped
    fn should_skip(&self, name: &str) -> bool {
        // Common test-related interfaces
        if self.skip_test_interfaces {
            let test_suffixes = ["Fake", "Mock", "Stub", "Test", "Spy"];
            if test_suffixes.iter().any(|s| name.ends_with(s)) {
                return true;
            }
        }

        // Repository/DataSource interfaces (debatable)
        if self.skip_repository_interfaces {
            let repo_patterns = ["Repository", "DataSource", "Gateway", "Service"];
            if repo_patterns.iter().any(|p| name.contains(p)) {
                return true;
            }
        }

        false
    }
}

impl Default for SingleImplInterfaceDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for SingleImplInterfaceDetector {
    fn detect(&self, graph: &Graph) -> Vec<DeadCode> {
        let mut issues = Vec::new();

        // Collect all interfaces
        let interfaces: Vec<_> = graph
            .declarations()
            .filter(|d| d.kind == DeclarationKind::Interface)
            .collect();

        // Count implementations for each interface
        let mut impl_count: HashMap<&str, usize> = HashMap::new();
        for interface in &interfaces {
            impl_count.insert(&interface.name, 0);
        }

        // Find all classes that implement interfaces
        for decl in graph.declarations() {
            if !matches!(decl.kind, DeclarationKind::Class) {
                continue;
            }

            // Check super_types for interface implementations
            for super_type in &decl.super_types {
                if let Some(count) = impl_count.get_mut(super_type.as_str()) {
                    *count += 1;
                }
            }
        }

        // Report interfaces with exactly 1 implementation
        for interface in interfaces {
            if self.should_skip(&interface.name) {
                continue;
            }

            let count = impl_count.get(interface.name.as_str()).unwrap_or(&0);
            if *count == 1 {
                let mut dead = DeadCode::new(interface.clone(), DeadCodeIssue::SingleImplInterface);
                dead = dead.with_message(format!(
                    "Interface '{}' has only 1 implementation. Consider using the class directly unless needed for testing.",
                    interface.name
                ));
                dead = dead.with_confidence(Confidence::Medium);
                issues.push(dead);
            }
        }

        // Sort by file and line
        issues.sort_by(|a, b| {
            a.declaration
                .location
                .file
                .cmp(&b.declaration.location.file)
                .then(
                    a.declaration
                        .location
                        .line
                        .cmp(&b.declaration.location.line),
                )
        });

        issues
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Declaration, DeclarationId, Language, Location};
    use std::path::PathBuf;

    fn create_interface(name: &str, line: usize) -> Declaration {
        let path = PathBuf::from("test.kt");
        Declaration::new(
            DeclarationId::new(path.clone(), line * 100, line * 100 + 50),
            name.to_string(),
            DeclarationKind::Interface,
            Location::new(path, line, 1, line * 100, line * 100 + 50),
            Language::Kotlin,
        )
    }

    fn create_class(name: &str, line: usize, implements: Vec<&str>) -> Declaration {
        let path = PathBuf::from("test.kt");
        let mut decl = Declaration::new(
            DeclarationId::new(path.clone(), line * 100, line * 100 + 50),
            name.to_string(),
            DeclarationKind::Class,
            Location::new(path, line, 1, line * 100, line * 100 + 50),
            Language::Kotlin,
        );
        decl.super_types = implements.into_iter().map(String::from).collect();
        decl
    }

    #[test]
    fn test_detector_creation() {
        let detector = SingleImplInterfaceDetector::new();
        assert!(detector.skip_test_interfaces);
        assert!(!detector.skip_repository_interfaces);
    }

    #[test]
    fn test_strict_mode() {
        let detector = SingleImplInterfaceDetector::new().strict();
        assert!(!detector.skip_test_interfaces);
        assert!(!detector.skip_repository_interfaces);
    }

    #[test]
    fn test_should_skip_test_interfaces() {
        let detector = SingleImplInterfaceDetector::new();
        assert!(detector.should_skip("UserRepositoryFake"));
        assert!(detector.should_skip("NetworkClientMock"));
        assert!(!detector.should_skip("UserRepository"));
    }

    #[test]
    fn test_empty_graph() {
        let graph = Graph::new();
        let detector = SingleImplInterfaceDetector::new();
        let issues = detector.detect(&graph);
        assert!(issues.is_empty());
    }

    #[test]
    fn test_single_implementation_detected() {
        let mut graph = Graph::new();
        graph.add_declaration(create_interface("UserService", 1));
        graph.add_declaration(create_class("UserServiceImpl", 5, vec!["UserService"]));

        let detector = SingleImplInterfaceDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].declaration.name, "UserService");
    }

    #[test]
    fn test_multiple_implementations_ok() {
        let mut graph = Graph::new();
        graph.add_declaration(create_interface("PaymentProcessor", 1));
        graph.add_declaration(create_class("StripeProcessor", 5, vec!["PaymentProcessor"]));
        graph.add_declaration(create_class("PayPalProcessor", 10, vec!["PaymentProcessor"]));

        let detector = SingleImplInterfaceDetector::new();
        let issues = detector.detect(&graph);

        assert!(issues.is_empty(), "Multiple implementations should be OK");
    }

    #[test]
    fn test_no_implementation() {
        let mut graph = Graph::new();
        graph.add_declaration(create_interface("OrphanInterface", 1));

        let detector = SingleImplInterfaceDetector::new();
        let issues = detector.detect(&graph);

        // No implementation = 0 impls, not 1, so should not trigger
        assert!(issues.is_empty());
    }
}
