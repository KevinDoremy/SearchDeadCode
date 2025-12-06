// XML parser module - some methods reserved for future use
#![allow(dead_code)]

mod layout;
mod manifest;
mod menu;
mod navigation;

pub use layout::LayoutParser;
pub use manifest::ManifestParser;
pub use menu::MenuParser;
pub use navigation::NavigationParser;

use std::collections::HashSet;

/// Result of parsing Android XML files
#[derive(Debug, Default)]
pub struct XmlParseResult {
    /// Class names referenced in the XML
    pub class_references: HashSet<String>,

    /// Package name from manifest
    pub package: Option<String>,
}

impl XmlParseResult {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn merge(&mut self, other: XmlParseResult) {
        self.class_references.extend(other.class_references);
        if self.package.is_none() {
            self.package = other.package;
        }
    }
}
