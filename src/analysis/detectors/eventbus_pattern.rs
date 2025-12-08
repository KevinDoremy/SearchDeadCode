//! EventBus Pattern Detector
//!
//! Detects usage of EventBus library or similar global event patterns.
//! This is considered an anti-pattern because it makes code flow unpredictable.
//!
//! ## Anti-Pattern
//!
//! ```kotlin
//! // Posting events that can be received anywhere
//! EventBus.getDefault().post(UserUpdatedEvent(userId))
//!
//! // Receiving events from unknown sources
//! @Subscribe(threadMode = ThreadMode.MAIN)
//! fun onUserUpdated(event: UserUpdatedEvent) { }
//! ```
//!
//! ## Why It's Bad
//!
//! - Implicit dependencies between components
//! - Hard to trace where events come from
//! - Difficult to debug and test
//! - Makes code flow unpredictable
//!
//! ## Better Alternatives
//!
//! - Direct method calls / callbacks
//! - StateFlow/SharedFlow for reactive streams
//! - Navigation component for navigation events
//! - ViewModel + LiveData for UI state

use super::Detector;
use crate::analysis::{Confidence, DeadCode, DeadCodeIssue};
use crate::graph::{DeclarationKind, Graph};

/// Detector for EventBus pattern usage
pub struct EventBusPatternDetector {
    /// EventBus-related annotations to detect
    eventbus_annotations: Vec<String>,
    /// EventBus-related class patterns
    eventbus_patterns: Vec<String>,
}

impl EventBusPatternDetector {
    pub fn new() -> Self {
        Self {
            eventbus_annotations: vec![
                "Subscribe".to_string(),
                "Subscriber".to_string(),
            ],
            eventbus_patterns: vec![
                "EventBus".to_string(),
                "Event".to_string(),  // Classes ending in Event
                "RxBus".to_string(),
                "MessageBus".to_string(),
            ],
        }
    }

    /// Check if a class name matches EventBus event pattern
    fn is_event_class(&self, name: &str) -> bool {
        // Classes named *Event are often EventBus events
        name.ends_with("Event") && !name.contains("Listener")
    }

    /// Check if declaration has EventBus annotations
    fn has_eventbus_annotation(&self, decl: &crate::graph::Declaration) -> bool {
        for annotation in &decl.annotations {
            for pattern in &self.eventbus_annotations {
                if annotation.contains(pattern) {
                    return true;
                }
            }
        }
        false
    }
}

impl Default for EventBusPatternDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for EventBusPatternDetector {
    fn detect(&self, graph: &Graph) -> Vec<DeadCode> {
        let mut issues = Vec::new();

        for decl in graph.declarations() {
            // Check for EventBus subscriber annotations
            if self.has_eventbus_annotation(decl) {
                let mut dead = DeadCode::new(decl.clone(), DeadCodeIssue::EventBusPattern);
                dead = dead.with_message(format!(
                    "Method '{}' uses EventBus @Subscribe annotation. Consider using StateFlow/callbacks instead.",
                    decl.name
                ));
                dead = dead.with_confidence(Confidence::High);
                issues.push(dead);
                continue;
            }

            // Check for Event classes
            if matches!(decl.kind, DeclarationKind::Class) && self.is_event_class(&decl.name) {
                // Skip if it's a UI event (like ClickEvent) or lifecycle event
                let skip_patterns = ["Click", "Touch", "Lifecycle", "State", "Action", "Intent"];
                if skip_patterns.iter().any(|p| decl.name.contains(p)) {
                    continue;
                }

                let mut dead = DeadCode::new(decl.clone(), DeadCodeIssue::EventBusPattern);
                dead = dead.with_message(format!(
                    "Class '{}' appears to be an EventBus event. Consider more structured communication patterns.",
                    decl.name
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

    fn create_method(name: &str, line: usize, annotations: Vec<&str>) -> Declaration {
        let path = PathBuf::from("test.kt");
        let mut decl = Declaration::new(
            DeclarationId::new(path.clone(), line * 100, line * 100 + 50),
            name.to_string(),
            DeclarationKind::Method,
            Location::new(path, line, 1, line * 100, line * 100 + 50),
            Language::Kotlin,
        );
        decl.annotations = annotations.into_iter().map(String::from).collect();
        decl
    }

    fn create_class(name: &str, line: usize) -> Declaration {
        let path = PathBuf::from("test.kt");
        Declaration::new(
            DeclarationId::new(path.clone(), line * 100, line * 100 + 50),
            name.to_string(),
            DeclarationKind::Class,
            Location::new(path, line, 1, line * 100, line * 100 + 50),
            Language::Kotlin,
        )
    }

    #[test]
    fn test_detector_creation() {
        let detector = EventBusPatternDetector::new();
        assert!(!detector.eventbus_annotations.is_empty());
        assert!(!detector.eventbus_patterns.is_empty());
    }

    #[test]
    fn test_is_event_class() {
        let detector = EventBusPatternDetector::new();
        assert!(detector.is_event_class("UserUpdatedEvent"));
        assert!(detector.is_event_class("DataRefreshEvent"));
        assert!(!detector.is_event_class("UserEventListener")); // Listener, not event
        assert!(!detector.is_event_class("UserService"));
    }

    #[test]
    fn test_has_eventbus_annotation() {
        let detector = EventBusPatternDetector::new();

        let with_subscribe = create_method("onEvent", 1, vec!["Subscribe"]);
        assert!(detector.has_eventbus_annotation(&with_subscribe));

        let without = create_method("onClick", 2, vec!["OnClick"]);
        assert!(!detector.has_eventbus_annotation(&without));
    }

    #[test]
    fn test_empty_graph() {
        let graph = Graph::new();
        let detector = EventBusPatternDetector::new();
        let issues = detector.detect(&graph);
        assert!(issues.is_empty());
    }

    #[test]
    fn test_detects_subscribe_annotation() {
        let mut graph = Graph::new();
        graph.add_declaration(create_method("onUserUpdated", 1, vec!["Subscribe"]));

        let detector = EventBusPatternDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].declaration.name, "onUserUpdated");
    }

    #[test]
    fn test_detects_event_class() {
        let mut graph = Graph::new();
        graph.add_declaration(create_class("UserUpdatedEvent", 1));

        let detector = EventBusPatternDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].declaration.name, "UserUpdatedEvent");
    }

    #[test]
    fn test_skips_ui_events() {
        let mut graph = Graph::new();
        graph.add_declaration(create_class("ButtonClickEvent", 1));
        graph.add_declaration(create_class("LifecycleEvent", 2));
        graph.add_declaration(create_class("UiStateEvent", 3));

        let detector = EventBusPatternDetector::new();
        let issues = detector.detect(&graph);

        assert!(issues.is_empty(), "UI-related events should be skipped");
    }
}
