use super::Detector;
use crate::analysis::DeadCode;
use crate::graph::Graph;

pub struct UnusedMethodDetector;

impl UnusedMethodDetector {
    pub fn new() -> Self {
        Self
    }
}

impl Detector for UnusedMethodDetector {
    fn detect(&self, _graph: &Graph) -> Vec<DeadCode> {
        Vec::new()
    }
}

impl Default for UnusedMethodDetector {
    fn default() -> Self {
        Self::new()
    }
}
