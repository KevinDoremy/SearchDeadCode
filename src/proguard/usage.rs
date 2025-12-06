// ProGuard/R8 usage.txt parser
//
// The usage.txt file lists all code that ProGuard/R8 determined was unused
// and removed (or would remove) during optimization.
//
// Format:
// ```
// com.example.UnusedClass
//     int unusedField
//     void unusedMethod(java.lang.String)
// com.example.PartiallyUsedClass
//     void onlyThisMethodIsUnused()
// ```

#![allow(dead_code)] // API methods reserved for future use

use miette::{IntoDiagnostic, Result};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

/// Represents parsed ProGuard usage.txt data
#[derive(Debug, Clone, Default)]
pub struct ProguardUsage {
    /// All unused entries indexed by fully qualified class name
    entries: HashMap<String, Vec<UsageEntry>>,
    /// Set of fully unused classes (entire class is dead)
    dead_classes: HashSet<String>,
    /// Total count of unused items
    pub total_count: usize,
}

/// A single entry from usage.txt
#[derive(Debug, Clone)]
pub struct UsageEntry {
    /// The class this entry belongs to
    pub class_name: String,
    /// The member name (method or field), None if entire class is unused
    pub member_name: Option<String>,
    /// Kind of entry
    pub kind: UsageEntryKind,
    /// Full signature (for methods)
    pub signature: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsageEntryKind {
    Class,
    Method,
    Field,
    Constructor,
}

impl ProguardUsage {
    /// Parse a usage.txt file
    pub fn parse(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path).into_diagnostic()?;
        Self::parse_content(&content)
    }

    /// Parse usage.txt content
    pub fn parse_content(content: &str) -> Result<Self> {
        let mut usage = ProguardUsage::default();
        let mut current_class: Option<String> = None;
        let mut class_has_members = false;

        for line in content.lines() {
            let line = line.trim_end();

            if line.is_empty() {
                continue;
            }

            // Lines starting with whitespace are members of the current class
            if line.starts_with(' ') || line.starts_with('\t') {
                let member_line = line.trim();
                if let Some(ref class_name) = current_class {
                    if let Some(entry) = Self::parse_member_line(class_name, member_line) {
                        usage.add_entry(entry);
                        class_has_members = true;
                    }
                }
            } else {
                // Before moving to next class, check if previous class had no members
                // (meaning the entire class is unused)
                if let Some(ref class_name) = current_class {
                    if !class_has_members {
                        usage.dead_classes.insert(class_name.clone());
                        usage.add_entry(UsageEntry {
                            class_name: class_name.clone(),
                            member_name: None,
                            kind: UsageEntryKind::Class,
                            signature: None,
                        });
                    }
                }

                // This is a class declaration
                current_class = Some(line.to_string());
                class_has_members = false;
            }
        }

        // Handle last class
        if let Some(ref class_name) = current_class {
            if !class_has_members {
                usage.dead_classes.insert(class_name.clone());
                usage.add_entry(UsageEntry {
                    class_name: class_name.clone(),
                    member_name: None,
                    kind: UsageEntryKind::Class,
                    signature: None,
                });
            }
        }

        Ok(usage)
    }

    /// Parse a member line (field or method)
    fn parse_member_line(class_name: &str, line: &str) -> Option<UsageEntry> {
        // Method pattern: "returnType methodName(params)"
        // Field pattern: "type fieldName"
        // Constructor pattern: "ClassName(params)"

        let line = line.trim();

        if line.contains('(') {
            // Method or constructor
            let is_constructor = !line.contains(' ')
                || line
                    .split_whitespace()
                    .next()
                    .map(|first| class_name.ends_with(first) || first == "<init>")
                    .unwrap_or(false);

            let name = if is_constructor {
                // Constructor: "ClassName(params)" or "<init>(params)"
                line.split('(').next().map(|s| s.trim().to_string())
            } else {
                // Method: "returnType methodName(params)"
                line.split('(')
                    .next()
                    .and_then(|before_paren| before_paren.split_whitespace().last())
                    .map(|s| s.to_string())
            };

            Some(UsageEntry {
                class_name: class_name.to_string(),
                member_name: name,
                kind: if is_constructor {
                    UsageEntryKind::Constructor
                } else {
                    UsageEntryKind::Method
                },
                signature: Some(line.to_string()),
            })
        } else {
            // Field: "type fieldName"
            let parts: Vec<&str> = line.split_whitespace().collect();
            let name = parts.last().map(|s| s.to_string());

            Some(UsageEntry {
                class_name: class_name.to_string(),
                member_name: name,
                kind: UsageEntryKind::Field,
                signature: Some(line.to_string()),
            })
        }
    }

    fn add_entry(&mut self, entry: UsageEntry) {
        self.total_count += 1;
        self.entries
            .entry(entry.class_name.clone())
            .or_default()
            .push(entry);
    }

    /// Check if a class is completely unused
    pub fn is_class_dead(&self, class_name: &str) -> bool {
        self.dead_classes.contains(class_name)
    }

    /// Check if a specific member is unused
    pub fn is_member_dead(&self, class_name: &str, member_name: &str) -> bool {
        if let Some(entries) = self.entries.get(class_name) {
            entries.iter().any(|e| {
                e.member_name
                    .as_ref()
                    .map(|n| n == member_name)
                    .unwrap_or(false)
            })
        } else {
            false
        }
    }

    /// Get all entries for a class
    pub fn get_class_entries(&self, class_name: &str) -> Option<&Vec<UsageEntry>> {
        self.entries.get(class_name)
    }

    /// Get all dead classes
    pub fn dead_classes(&self) -> &HashSet<String> {
        &self.dead_classes
    }

    /// Get all entries
    pub fn all_entries(&self) -> impl Iterator<Item = &UsageEntry> {
        self.entries.values().flatten()
    }

    /// Get statistics
    pub fn stats(&self) -> UsageStats {
        let mut classes = 0;
        let mut methods = 0;
        let mut fields = 0;
        let mut constructors = 0;

        for entry in self.all_entries() {
            match entry.kind {
                UsageEntryKind::Class => classes += 1,
                UsageEntryKind::Method => methods += 1,
                UsageEntryKind::Field => fields += 1,
                UsageEntryKind::Constructor => constructors += 1,
            }
        }

        UsageStats {
            total: self.total_count,
            classes,
            methods,
            fields,
            constructors,
        }
    }

    /// Convert to simple name lookup (for matching with our declarations)
    pub fn to_simple_name_set(&self) -> HashSet<String> {
        let mut names = HashSet::new();

        for entry in self.all_entries() {
            // Add class simple name
            if let Some(simple) = entry.class_name.split('.').next_back() {
                names.insert(simple.to_string());
            }

            // Add member name
            if let Some(ref member) = entry.member_name {
                names.insert(member.clone());
            }
        }

        names
    }

    /// Match against a declaration name and return confidence boost
    pub fn get_confidence_for(&self, class_name: Option<&str>, member_name: &str) -> Option<f64> {
        // Check if this exact member is in usage.txt
        if let Some(class) = class_name {
            if self.is_class_dead(class) {
                return Some(1.0); // Entire class is dead - confirmed
            }
            if self.is_member_dead(class, member_name) {
                return Some(1.0); // This specific member is dead - confirmed
            }
        }

        // Check by simple name matching (less confident)
        for entry in self.all_entries() {
            if let Some(ref name) = entry.member_name {
                if name == member_name {
                    return Some(0.8); // Name matches but might be different class
                }
            }
            if let Some(simple) = entry.class_name.split('.').next_back() {
                if simple == member_name {
                    return Some(0.8);
                }
            }
        }

        None
    }
}

#[derive(Debug, Clone)]
pub struct UsageStats {
    pub total: usize,
    pub classes: usize,
    pub methods: usize,
    pub fields: usize,
    pub constructors: usize,
}

impl std::fmt::Display for UsageStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} total ({} classes, {} methods, {} fields, {} constructors)",
            self.total, self.classes, self.methods, self.fields, self.constructors
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_usage_txt() {
        let content = r#"
com.example.UnusedClass
com.example.PartiallyUsed
    int unusedField
    void unusedMethod(java.lang.String)
    void anotherMethod()
com.example.AnotherUnused
"#;
        let usage = ProguardUsage::parse_content(content).unwrap();

        assert!(usage.is_class_dead("com.example.UnusedClass"));
        assert!(usage.is_class_dead("com.example.AnotherUnused"));
        assert!(!usage.is_class_dead("com.example.PartiallyUsed"));

        assert!(usage.is_member_dead("com.example.PartiallyUsed", "unusedField"));
        assert!(usage.is_member_dead("com.example.PartiallyUsed", "unusedMethod"));

        let stats = usage.stats();
        assert_eq!(stats.classes, 2);
        assert_eq!(stats.methods, 2);
        assert_eq!(stats.fields, 1);
    }

    #[test]
    fn test_parse_constructor() {
        let content = r#"
com.example.MyClass
    MyClass(java.lang.String)
    void myMethod()
"#;
        let usage = ProguardUsage::parse_content(content).unwrap();
        let stats = usage.stats();

        assert_eq!(stats.constructors, 1);
        assert_eq!(stats.methods, 1);
    }
}
