//! Heavy ViewModel Detector
//!
//! Detects ViewModels with too many dependencies or direct data layer access.
//! This is a common anti-pattern known as "God ViewModel".
//!
//! ## Anti-Pattern
//!
//! ```kotlin
//! class UserViewModel @Inject constructor(
//!     private val userRepository: UserRepository,
//!     private val settingsRepository: SettingsRepository,
//!     private val analyticsManager: AnalyticsManager,
//!     private val notificationManager: NotificationManager,
//!     private val cacheManager: CacheManager,
//!     private val networkMonitor: NetworkMonitor,
//!     private val featureFlags: FeatureFlags,
//!     private val logger: Logger,
//!     private val errorHandler: ErrorHandler  // Too many dependencies!
//! ) : ViewModel()
//! ```
//!
//! ## Why It's Bad
//!
//! - Violates Single Responsibility Principle
//! - Hard to test (many mocks needed)
//! - Hard to understand and maintain
//! - Indicates ViewModel is doing too much
//!
//! ## Better Alternatives
//!
//! - Split into multiple smaller ViewModels
//! - Use use cases / interactors to encapsulate business logic
//! - Facade pattern to group related dependencies

use super::Detector;
use crate::analysis::{Confidence, DeadCode, DeadCodeIssue};
use crate::graph::{DeclarationKind, Graph};
use std::collections::HashMap;

/// Detector for ViewModels with too many dependencies
pub struct HeavyViewModelDetector {
    /// Maximum allowed dependencies (constructor parameters) before warning
    max_dependencies: usize,
    /// Maximum allowed methods before warning
    max_methods: usize,
    /// Direct data layer patterns to detect
    direct_data_patterns: Vec<String>,
}

impl HeavyViewModelDetector {
    pub fn new() -> Self {
        Self {
            max_dependencies: 6,
            max_methods: 15,
            direct_data_patterns: vec![
                "Database".to_string(),
                "Retrofit".to_string(),
                "SharedPreferences".to_string(),
                "DataStore".to_string(),
                "Dao".to_string(),
                "ApiService".to_string(),
                "HttpClient".to_string(),
            ],
        }
    }

    /// Set maximum dependencies before warning
    #[allow(dead_code)]
    pub fn with_max_dependencies(mut self, max: usize) -> Self {
        self.max_dependencies = max;
        self
    }

    /// Set maximum methods before warning
    #[allow(dead_code)]
    pub fn with_max_methods(mut self, max: usize) -> Self {
        self.max_methods = max;
        self
    }

    /// Check if a class is a ViewModel
    fn is_viewmodel(&self, decl: &crate::graph::Declaration) -> bool {
        decl.super_types
            .iter()
            .any(|s| s.contains("ViewModel") || s.contains("AndroidViewModel"))
    }

    /// Check if a type name indicates direct data layer access
    fn is_direct_data_access(&self, type_name: &str) -> bool {
        self.direct_data_patterns
            .iter()
            .any(|p| type_name.contains(p))
    }
}

impl Default for HeavyViewModelDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for HeavyViewModelDetector {
    fn detect(&self, graph: &Graph) -> Vec<DeadCode> {
        let mut issues: Vec<DeadCode> = Vec::new();

        // Group declarations by parent to count methods per ViewModel
        let mut viewmodel_children: HashMap<&crate::graph::DeclarationId, Vec<&crate::graph::Declaration>> =
            HashMap::new();

        // First pass: identify ViewModels and collect their children
        let viewmodels: Vec<_> = graph
            .declarations()
            .filter(|d| matches!(d.kind, DeclarationKind::Class) && self.is_viewmodel(d))
            .collect();

        // Second pass: group children by parent
        for decl in graph.declarations() {
            if let Some(ref parent_id) = decl.parent {
                viewmodel_children
                    .entry(parent_id)
                    .or_default()
                    .push(decl);
            }
        }

        // Analyze each ViewModel
        for vm in &viewmodels {
            let children = viewmodel_children.get(&vm.id).map(|v| v.as_slice()).unwrap_or(&[]);

            // Count constructor parameters
            let param_count = children
                .iter()
                .filter(|c| matches!(c.kind, DeclarationKind::Parameter))
                .count();

            // Count methods
            let method_count = children
                .iter()
                .filter(|c| matches!(c.kind, DeclarationKind::Method))
                .count();

            // Check for direct data layer access in dependencies
            let direct_data_deps: Vec<_> = children
                .iter()
                .filter(|c| {
                    matches!(c.kind, DeclarationKind::Parameter | DeclarationKind::Property)
                        && self.is_direct_data_access(&c.name)
                })
                .map(|c| c.name.as_str())
                .collect();

            // Check for too many dependencies
            if param_count > self.max_dependencies {
                let mut dead = DeadCode::new((*vm).clone(), DeadCodeIssue::HeavyViewModel);
                dead = dead.with_message(format!(
                    "ViewModel '{}' has {} constructor parameters (max recommended: {}). Consider splitting into smaller ViewModels.",
                    vm.name, param_count, self.max_dependencies
                ));
                dead = dead.with_confidence(Confidence::Medium);
                issues.push(dead);
            }
            // Check for too many methods
            else if method_count > self.max_methods {
                let mut dead = DeadCode::new((*vm).clone(), DeadCodeIssue::HeavyViewModel);
                dead = dead.with_message(format!(
                    "ViewModel '{}' has {} methods (max recommended: {}). Consider splitting responsibilities.",
                    vm.name, method_count, self.max_methods
                ));
                dead = dead.with_confidence(Confidence::Medium);
                issues.push(dead);
            }
            // Check for direct data layer access
            else if !direct_data_deps.is_empty() {
                let mut dead = DeadCode::new((*vm).clone(), DeadCodeIssue::HeavyViewModel);
                dead = dead.with_message(format!(
                    "ViewModel '{}' has direct data layer access ({}). Consider using repository pattern.",
                    vm.name,
                    direct_data_deps.join(", ")
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
            DeclarationId::new(path.clone(), line * 100, line * 100 + 50),
            name.to_string(),
            DeclarationKind::Class,
            Location::new(path, line, 1, line * 100, line * 100 + 50),
            Language::Kotlin,
        );
        decl.super_types = vec!["ViewModel".to_string()];
        decl
    }

    fn create_regular_class(name: &str, line: usize) -> Declaration {
        let path = PathBuf::from("test.kt");
        Declaration::new(
            DeclarationId::new(path.clone(), line * 100, line * 100 + 50),
            name.to_string(),
            DeclarationKind::Class,
            Location::new(path, line, 1, line * 100, line * 100 + 50),
            Language::Kotlin,
        )
    }

    fn create_parameter(name: &str, parent_id: DeclarationId, line: usize) -> Declaration {
        let path = PathBuf::from("test.kt");
        let mut decl = Declaration::new(
            DeclarationId::new(path.clone(), line * 100, line * 100 + 20),
            name.to_string(),
            DeclarationKind::Parameter,
            Location::new(path, line, 1, line * 100, line * 100 + 20),
            Language::Kotlin,
        );
        decl.parent = Some(parent_id);
        decl
    }

    fn create_method(name: &str, parent_id: DeclarationId, line: usize) -> Declaration {
        let path = PathBuf::from("test.kt");
        let mut decl = Declaration::new(
            DeclarationId::new(path.clone(), line * 100, line * 100 + 30),
            name.to_string(),
            DeclarationKind::Method,
            Location::new(path, line, 1, line * 100, line * 100 + 30),
            Language::Kotlin,
        );
        decl.parent = Some(parent_id);
        decl
    }

    #[test]
    fn test_detector_creation() {
        let detector = HeavyViewModelDetector::new();
        assert_eq!(detector.max_dependencies, 6);
        assert_eq!(detector.max_methods, 15);
    }

    #[test]
    fn test_with_max_dependencies() {
        let detector = HeavyViewModelDetector::new().with_max_dependencies(10);
        assert_eq!(detector.max_dependencies, 10);
    }

    #[test]
    fn test_is_viewmodel() {
        let detector = HeavyViewModelDetector::new();
        let vm = create_viewmodel("UserViewModel", 1);
        let regular = create_regular_class("UserService", 2);

        assert!(detector.is_viewmodel(&vm));
        assert!(!detector.is_viewmodel(&regular));
    }

    #[test]
    fn test_is_direct_data_access() {
        let detector = HeavyViewModelDetector::new();
        assert!(detector.is_direct_data_access("AppDatabase"));
        assert!(detector.is_direct_data_access("UserDao"));
        assert!(detector.is_direct_data_access("Retrofit"));
        assert!(detector.is_direct_data_access("SharedPreferences"));
        assert!(!detector.is_direct_data_access("UserRepository"));
        assert!(!detector.is_direct_data_access("AnalyticsManager"));
    }

    #[test]
    fn test_empty_graph() {
        let graph = Graph::new();
        let detector = HeavyViewModelDetector::new();
        let issues = detector.detect(&graph);
        assert!(issues.is_empty());
    }

    #[test]
    fn test_viewmodel_within_limits() {
        let mut graph = Graph::new();
        let vm = create_viewmodel("SimpleViewModel", 1);
        let vm_id = vm.id.clone();
        graph.add_declaration(vm);
        graph.add_declaration(create_parameter("repo", vm_id.clone(), 2));
        graph.add_declaration(create_parameter("analytics", vm_id.clone(), 3));
        graph.add_declaration(create_parameter("handler", vm_id, 4));

        let detector = HeavyViewModelDetector::new();
        let issues = detector.detect(&graph);

        assert!(issues.is_empty(), "ViewModel with 3 params should be OK");
    }

    #[test]
    fn test_viewmodel_too_many_parameters() {
        let mut graph = Graph::new();
        let vm = create_viewmodel("HeavyViewModel", 1);
        let vm_id = vm.id.clone();
        graph.add_declaration(vm);

        for i in 0..8 {
            graph.add_declaration(create_parameter(&format!("dep{}", i), vm_id.clone(), 2 + i));
        }

        let detector = HeavyViewModelDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].declaration.name, "HeavyViewModel");
        assert!(issues[0].message.contains("8 constructor parameters"));
    }

    #[test]
    fn test_viewmodel_too_many_methods() {
        let mut graph = Graph::new();
        let vm = create_viewmodel("MethodHeavyViewModel", 1);
        let vm_id = vm.id.clone();
        graph.add_declaration(vm);

        for i in 0..20 {
            graph.add_declaration(create_method(&format!("method{}", i), vm_id.clone(), 2 + i));
        }

        let detector = HeavyViewModelDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
        assert!(issues[0].message.contains("20 methods"));
    }

    #[test]
    fn test_regular_class_not_flagged() {
        let mut graph = Graph::new();
        let svc = create_regular_class("HeavyService", 1);
        let svc_id = svc.id.clone();
        graph.add_declaration(svc);

        for i in 0..10 {
            graph.add_declaration(create_parameter(&format!("dep{}", i), svc_id.clone(), 2 + i));
        }

        let detector = HeavyViewModelDetector::new();
        let issues = detector.detect(&graph);

        assert!(
            issues.is_empty(),
            "Non-ViewModel classes should not be flagged"
        );
    }
}
