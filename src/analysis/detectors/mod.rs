// Detectors for specific types of dead code
// These can be extended for more advanced analysis
#![allow(dead_code)]
#![allow(unused_imports)]

mod assign_only;
mod dead_branch;
mod ignored_return;
mod redundant_override;
mod redundant_public;
mod sealed_variant;
mod unused_class;
mod unused_enum_case;
mod unused_import;
mod unused_intent_extra;
mod unused_method;
mod unused_param;
mod unused_property;
mod write_only;

// These detectors are reserved for future advanced analysis modes
pub use assign_only::AssignOnlyDetector;
pub use dead_branch::DeadBranchDetector;
pub use ignored_return::IgnoredReturnValueDetector;
pub use redundant_override::RedundantOverrideDetector;
pub use redundant_public::RedundantPublicDetector;
pub use sealed_variant::UnusedSealedVariantDetector;
pub use unused_class::UnusedClassDetector;
pub use unused_enum_case::UnusedEnumCaseDetector;
pub use unused_import::UnusedImportDetector;
pub use unused_intent_extra::{ExtraLocation, IntentExtraAnalysis, UnusedIntentExtraDetector};
pub use unused_method::UnusedMethodDetector;
pub use unused_param::UnusedParamDetector;
pub use unused_property::UnusedPropertyDetector;
pub use write_only::WriteOnlyDetector;

use crate::analysis::DeadCode;
use crate::graph::Graph;

/// Trait for dead code detectors
pub trait Detector {
    /// Run the detector on the graph and return found issues
    fn detect(&self, graph: &Graph) -> Vec<DeadCode>;
}
