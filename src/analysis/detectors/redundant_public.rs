use super::Detector;
use crate::analysis::DeadCode;
use crate::graph::Graph;

pub struct RedundantPublicDetector;
impl RedundantPublicDetector {
    pub fn new() -> Self {
        Self
    }
}
impl Detector for RedundantPublicDetector {
    fn detect(&self, _graph: &Graph) -> Vec<DeadCode> {
        Vec::new()
    }
}
impl Default for RedundantPublicDetector {
    fn default() -> Self {
        Self::new()
    }
}
