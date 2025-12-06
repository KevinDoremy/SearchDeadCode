use super::Detector;
use crate::analysis::DeadCode;
use crate::graph::Graph;

pub struct AssignOnlyDetector;
impl AssignOnlyDetector {
    pub fn new() -> Self {
        Self
    }
}
impl Detector for AssignOnlyDetector {
    fn detect(&self, _graph: &Graph) -> Vec<DeadCode> {
        Vec::new()
    }
}
impl Default for AssignOnlyDetector {
    fn default() -> Self {
        Self::new()
    }
}
