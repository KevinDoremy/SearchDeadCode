use super::Detector;
use crate::analysis::DeadCode;
use crate::graph::Graph;

pub struct DeadBranchDetector;
impl DeadBranchDetector {
    pub fn new() -> Self {
        Self
    }
}
impl Detector for DeadBranchDetector {
    fn detect(&self, _graph: &Graph) -> Vec<DeadCode> {
        Vec::new()
    }
}
impl Default for DeadBranchDetector {
    fn default() -> Self {
        Self::new()
    }
}
