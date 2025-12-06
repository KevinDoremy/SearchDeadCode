use super::Detector;
use crate::analysis::DeadCode;
use crate::graph::Graph;

pub struct UnusedImportDetector;
impl UnusedImportDetector {
    pub fn new() -> Self {
        Self
    }
}
impl Detector for UnusedImportDetector {
    fn detect(&self, _graph: &Graph) -> Vec<DeadCode> {
        Vec::new()
    }
}
impl Default for UnusedImportDetector {
    fn default() -> Self {
        Self::new()
    }
}
