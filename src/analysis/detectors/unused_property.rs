use super::Detector;
use crate::analysis::DeadCode;
use crate::graph::Graph;

pub struct UnusedPropertyDetector;
impl UnusedPropertyDetector {
    pub fn new() -> Self {
        Self
    }
}
impl Detector for UnusedPropertyDetector {
    fn detect(&self, _graph: &Graph) -> Vec<DeadCode> {
        Vec::new()
    }
}
impl Default for UnusedPropertyDetector {
    fn default() -> Self {
        Self::new()
    }
}
