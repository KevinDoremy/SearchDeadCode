// ProGuard/R8 integration module
//
// Parses ProGuard/R8 output files to enhance dead code detection:
// - usage.txt: Lists code that ProGuard determined is unused
// - seeds.txt: Lists code that matched -keep rules
// - mapping.txt: Obfuscation mapping (for reverse lookups)

mod report_generator;
mod usage;

pub use report_generator::ReportGenerator;
pub use usage::{ProguardUsage, UsageEntryKind};
