// Detectors for specific types of dead code
// These can be extended for more advanced analysis
#![allow(dead_code)]
#![allow(unused_imports)]

mod assign_only;
mod dead_branch;
mod duplicate_import;
mod ignored_return;
mod prefer_isempty;
mod redundant_null_init;
mod redundant_override;
mod redundant_parens;
mod redundant_public;
mod redundant_this;
mod sealed_variant;
mod unused_class;
mod unused_enum_case;
mod unused_import;
mod unused_intent_extra;
mod unused_method;
mod unused_param;
mod unused_property;
mod write_only;
mod write_only_dao;
mod write_only_prefs;

// Anti-pattern detectors (inspired by "8 anti-patterns in Android codebase")
mod deep_inheritance;
mod eventbus_pattern;
mod global_mutable_state;
mod single_impl_interface;

// Phase 2 anti-pattern detectors (from Kotlin/Android research)
mod globalscope_usage;
mod heavy_viewmodel;
mod lateinit_abuse;
mod scope_function_chaining;

// These detectors are reserved for future advanced analysis modes
pub use assign_only::AssignOnlyDetector;
pub use dead_branch::DeadBranchDetector;
pub use duplicate_import::DuplicateImportDetector;
pub use ignored_return::IgnoredReturnValueDetector;
pub use prefer_isempty::PreferIsEmptyDetector;
pub use redundant_null_init::RedundantNullInitDetector;
pub use redundant_override::RedundantOverrideDetector;
pub use redundant_parens::RedundantParenthesesDetector;
pub use redundant_public::RedundantPublicDetector;
pub use redundant_this::RedundantThisDetector;
pub use sealed_variant::UnusedSealedVariantDetector;
pub use unused_class::UnusedClassDetector;
pub use unused_enum_case::UnusedEnumCaseDetector;
pub use unused_import::UnusedImportDetector;
pub use unused_intent_extra::{ExtraLocation, IntentExtraAnalysis, UnusedIntentExtraDetector};
pub use unused_method::UnusedMethodDetector;
pub use unused_param::UnusedParamDetector;
pub use unused_property::UnusedPropertyDetector;
pub use write_only::WriteOnlyDetector;
pub use write_only_dao::{DaoAnalysis, DaoCollectionAnalysis, WriteOnlyDaoDetector};
pub use write_only_prefs::{SharedPrefsAnalysis, WriteOnlyPrefsDetector};

// Anti-pattern detectors
pub use deep_inheritance::DeepInheritanceDetector;
pub use eventbus_pattern::EventBusPatternDetector;
pub use global_mutable_state::GlobalMutableStateDetector;
pub use single_impl_interface::SingleImplInterfaceDetector;

// Phase 2 anti-pattern detectors
pub use globalscope_usage::GlobalScopeUsageDetector;
pub use heavy_viewmodel::HeavyViewModelDetector;
pub use lateinit_abuse::LateinitAbuseDetector;
pub use scope_function_chaining::ScopeFunctionChainingDetector;

use crate::analysis::DeadCode;
use crate::graph::Graph;

/// Trait for dead code detectors
pub trait Detector {
    /// Run the detector on the graph and return found issues
    fn detect(&self, graph: &Graph) -> Vec<DeadCode>;
}
