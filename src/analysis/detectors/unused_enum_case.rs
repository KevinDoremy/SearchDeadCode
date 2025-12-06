use super::Detector;
use crate::analysis::DeadCode;
use crate::graph::Graph;

pub struct UnusedEnumCaseDetector;
impl UnusedEnumCaseDetector {
    pub fn new() -> Self {
        Self
    }
}
impl Detector for UnusedEnumCaseDetector {
    fn detect(&self, _graph: &Graph) -> Vec<DeadCode> {
        Vec::new()
    }
}
impl Default for UnusedEnumCaseDetector {
    fn default() -> Self {
        Self::new()
    }
}
