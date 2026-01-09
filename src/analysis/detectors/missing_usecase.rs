//! Missing UseCase Detector
//!
//! Detects ViewModels that directly depend on Repositories
//! without using UseCases/Interactors (missing domain layer).
//!
//! ## Anti-Pattern
//!
//! ```kotlin
//! class BadViewModel(
//!     private val userRepository: UserRepository,
//!     private val orderRepository: OrderRepository,
//!     private val productRepository: ProductRepository
//! ) : ViewModel()
//! ```
//!
//! ## Why It's Bad
//!
//! - Business logic mixed with presentation
//! - Hard to test complex flows
//! - Violates Clean Architecture
//! - Multiple repositories = orchestration needed
//!
//! ## Better Alternatives
//!
//! ```kotlin
//! class GoodViewModel(
//!     private val getUserUseCase: GetUserUseCase,
//!     private val loadDashboardUseCase: LoadDashboardUseCase
//! ) : ViewModel()
//! ```

use super::Detector;
use crate::analysis::{Confidence, DeadCode, DeadCodeIssue};
use crate::graph::{DeclarationKind, Graph};

/// Detector for missing domain layer (UseCase/Interactor)
pub struct MissingUseCaseDetector {
    /// Threshold for number of repositories before warning
    max_repositories: usize,
}

impl MissingUseCaseDetector {
    pub fn new() -> Self {
        Self { max_repositories: 2 }
    }

    /// Set maximum repositories before warning
    #[allow(dead_code)]
    pub fn with_max_repositories(mut self, max: usize) -> Self {
        self.max_repositories = max;
        self
    }

    /// Check if class is a ViewModel
    fn is_viewmodel_class(decl: &crate::graph::Declaration) -> bool {
        let name_lower = decl.name.to_lowercase();
        name_lower.contains("viewmodel")
            || decl
                .super_types
                .iter()
                .any(|s| s.to_lowercase().contains("viewmodel"))
    }

    /// Check if property name suggests a Repository
    fn is_repository_property(name: &str) -> bool {
        let lower = name.to_lowercase();
        lower.contains("repository") || lower.contains("repo")
    }

    /// Check if property name suggests a UseCase/Interactor
    fn is_usecase_property(name: &str) -> bool {
        let lower = name.to_lowercase();
        lower.contains("usecase") || lower.contains("interactor")
    }
}

impl Default for MissingUseCaseDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for MissingUseCaseDetector {
    fn detect(&self, graph: &Graph) -> Vec<DeadCode> {
        let mut issues: Vec<DeadCode> = Vec::new();

        // Find all ViewModels
        for vm in graph.declarations() {
            if !matches!(vm.kind, DeclarationKind::Class) {
                continue;
            }

            if !Self::is_viewmodel_class(vm) {
                continue;
            }

            // Count repository and usecase properties
            let children = graph.get_children(&vm.id);
            let mut repo_count = 0;
            let mut has_usecase = false;

            for child_id in &children {
                if let Some(child) = graph.get_declaration(child_id) {
                    if matches!(child.kind, DeclarationKind::Property | DeclarationKind::Field) {
                        if Self::is_repository_property(&child.name) {
                            repo_count += 1;
                        }
                        if Self::is_usecase_property(&child.name) {
                            has_usecase = true;
                        }
                    }
                }
            }

            // Flag if has repositories but no usecases, and exceeds threshold
            if repo_count > self.max_repositories && !has_usecase {
                let mut dead = DeadCode::new(vm.clone(), DeadCodeIssue::MissingUseCase);
                dead = dead.with_message(format!(
                    "ViewModel '{}' has {} repository dependencies but no UseCase/Interactor. Consider adding a domain layer for business logic.",
                    vm.name, repo_count
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

    fn create_viewmodel(name: &str, line: usize) -> Declaration {
        let path = PathBuf::from("test.kt");
        let mut decl = Declaration::new(
            DeclarationId::new(path.clone(), line * 100, line * 100 + 500),
            name.to_string(),
            DeclarationKind::Class,
            Location::new(path, line, 1, line * 100, line * 100 + 500),
            Language::Kotlin,
        );
        decl.super_types.push("ViewModel".to_string());
        decl
    }

    fn create_property_with_parent(
        name: &str,
        parent_id: DeclarationId,
        line: usize,
    ) -> Declaration {
        let path = PathBuf::from("test.kt");
        let mut decl = Declaration::new(
            DeclarationId::new(path.clone(), line * 100, line * 100 + 50),
            name.to_string(),
            DeclarationKind::Property,
            Location::new(path, line, 1, line * 100, line * 100 + 50),
            Language::Kotlin,
        );
        decl.parent = Some(parent_id);
        decl
    }

    #[test]
    fn test_detector_creation() {
        let detector = MissingUseCaseDetector::new();
        assert_eq!(detector.max_repositories, 2);
    }

    #[test]
    fn test_with_max_repositories() {
        let detector = MissingUseCaseDetector::new().with_max_repositories(3);
        assert_eq!(detector.max_repositories, 3);
    }

    #[test]
    fn test_empty_graph() {
        let graph = Graph::new();
        let detector = MissingUseCaseDetector::new();
        let issues = detector.detect(&graph);
        assert!(issues.is_empty());
    }

    #[test]
    fn test_many_repositories_detected() {
        let mut graph = Graph::new();
        let vm = create_viewmodel("DashboardViewModel", 1);
        let vm_id = vm.id.clone();
        graph.add_declaration(vm);
        graph.add_declaration(create_property_with_parent("userRepository", vm_id.clone(), 2));
        graph.add_declaration(create_property_with_parent("orderRepository", vm_id.clone(), 3));
        graph.add_declaration(create_property_with_parent(
            "productRepository",
            vm_id.clone(),
            4,
        ));

        let detector = MissingUseCaseDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
        assert!(issues[0].message.contains("3 repository"));
    }

    #[test]
    fn test_with_usecase_ok() {
        let mut graph = Graph::new();
        let vm = create_viewmodel("DashboardViewModel", 1);
        let vm_id = vm.id.clone();
        graph.add_declaration(vm);
        graph.add_declaration(create_property_with_parent("userRepository", vm_id.clone(), 2));
        graph.add_declaration(create_property_with_parent("orderRepository", vm_id.clone(), 3));
        graph.add_declaration(create_property_with_parent(
            "productRepository",
            vm_id.clone(),
            4,
        ));
        graph.add_declaration(create_property_with_parent(
            "loadDashboardUseCase",
            vm_id.clone(),
            5,
        ));

        let detector = MissingUseCaseDetector::new();
        let issues = detector.detect(&graph);

        assert!(issues.is_empty(), "ViewModels with UseCases should be OK");
    }

    #[test]
    fn test_single_repository_ok() {
        let mut graph = Graph::new();
        let vm = create_viewmodel("SettingsViewModel", 1);
        let vm_id = vm.id.clone();
        graph.add_declaration(vm);
        graph.add_declaration(create_property_with_parent(
            "settingsRepository",
            vm_id.clone(),
            2,
        ));

        let detector = MissingUseCaseDetector::new();
        let issues = detector.detect(&graph);

        assert!(issues.is_empty(), "Single repository is acceptable");
    }
}
