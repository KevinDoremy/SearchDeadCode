// File discovery utilities - some reserved for future use
#![allow(dead_code)]

use crate::config::Config;
use ignore::WalkBuilder;
use miette::{IntoDiagnostic, Result};
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use tracing::{debug, trace};

/// Type of source file
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileType {
    Kotlin,
    Java,
    XmlManifest,
    XmlLayout,
    XmlNavigation,
    XmlMenu,
    XmlOther,
}

impl FileType {
    /// Determine file type from path
    pub fn from_path(path: &Path) -> Option<Self> {
        let extension = path.extension()?.to_str()?;
        let file_name = path.file_name()?.to_str()?;

        match extension {
            "kt" | "kts" => Some(FileType::Kotlin),
            "java" => Some(FileType::Java),
            "xml" => {
                // Determine XML type based on path
                let path_str = path.to_string_lossy();
                if file_name == "AndroidManifest.xml" {
                    Some(FileType::XmlManifest)
                } else if path_str.contains("/res/layout") || path_str.contains("\\res\\layout") {
                    Some(FileType::XmlLayout)
                } else if path_str.contains("/res/navigation")
                    || path_str.contains("\\res\\navigation")
                {
                    Some(FileType::XmlNavigation)
                } else if path_str.contains("/res/menu") || path_str.contains("\\res\\menu") {
                    Some(FileType::XmlMenu)
                } else {
                    Some(FileType::XmlOther)
                }
            }
            _ => None,
        }
    }

    /// Check if this is a source code file (Kotlin or Java)
    pub fn is_source(&self) -> bool {
        matches!(self, FileType::Kotlin | FileType::Java)
    }

    /// Check if this is an XML file
    pub fn is_xml(&self) -> bool {
        matches!(
            self,
            FileType::XmlManifest
                | FileType::XmlLayout
                | FileType::XmlNavigation
                | FileType::XmlMenu
                | FileType::XmlOther
        )
    }
}

/// Represents a discovered source file
#[derive(Debug, Clone)]
pub struct SourceFile {
    /// Absolute path to the file
    pub path: PathBuf,

    /// Type of source file
    pub file_type: FileType,

    /// Contents of the file (loaded lazily)
    contents: Option<String>,
}

impl SourceFile {
    pub fn new(path: PathBuf, file_type: FileType) -> Self {
        Self {
            path,
            file_type,
            contents: None,
        }
    }

    /// Load file contents
    pub fn load(&mut self) -> Result<&str> {
        if self.contents.is_none() {
            let contents = std::fs::read_to_string(&self.path).into_diagnostic()?;
            self.contents = Some(contents);
        }
        Ok(self.contents.as_ref().unwrap())
    }

    /// Get contents if already loaded
    pub fn contents(&self) -> Option<&str> {
        self.contents.as_deref()
    }

    /// Load and return owned contents
    pub fn read_contents(&self) -> Result<String> {
        std::fs::read_to_string(&self.path).into_diagnostic()
    }
}

/// File finder for discovering source files in a project
pub struct FileFinder<'a> {
    config: &'a Config,
}

impl<'a> FileFinder<'a> {
    pub fn new(config: &'a Config) -> Self {
        Self { config }
    }

    /// Find all source files in the given path
    pub fn find_files(&self, root: &Path) -> Result<Vec<SourceFile>> {
        debug!("Scanning for files in: {}", root.display());

        let targets = if self.config.targets.is_empty() {
            vec![root.to_path_buf()]
        } else {
            self.config.targets.iter().map(|t| root.join(t)).collect()
        };

        let files: Vec<SourceFile> = targets
            .par_iter()
            .flat_map(|target| self.scan_directory(target))
            .collect();

        debug!("Found {} files", files.len());
        Ok(files)
    }

    /// Scan a single directory for source files
    fn scan_directory(&self, dir: &Path) -> Vec<SourceFile> {
        if !dir.exists() {
            trace!("Directory does not exist: {}", dir.display());
            return Vec::new();
        }

        let walker = WalkBuilder::new(dir)
            .hidden(true) // Skip hidden files
            .git_ignore(true) // Respect .gitignore
            .git_global(true) // Respect global gitignore
            .git_exclude(true) // Respect .git/info/exclude
            .ignore(true) // Respect .ignore files
            .parents(true) // Check parent directories for ignore files
            .follow_links(false) // Don't follow symlinks
            .build();

        walker
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_type().map(|t| t.is_file()).unwrap_or(false))
            .filter_map(|entry| {
                let path = entry.path();

                // Check exclusion patterns
                if self.config.should_exclude(path) {
                    trace!("Excluding: {}", path.display());
                    return None;
                }

                // Determine file type
                let file_type = FileType::from_path(path)?;

                trace!("Found {:?}: {}", file_type, path.display());
                Some(SourceFile::new(path.to_path_buf(), file_type))
            })
            .collect()
    }

    /// Find only Kotlin and Java source files
    pub fn find_source_files(&self, root: &Path) -> Result<Vec<SourceFile>> {
        let files = self.find_files(root)?;
        Ok(files
            .into_iter()
            .filter(|f| f.file_type.is_source())
            .collect())
    }

    /// Find only XML files
    pub fn find_xml_files(&self, root: &Path) -> Result<Vec<SourceFile>> {
        let files = self.find_files(root)?;
        Ok(files.into_iter().filter(|f| f.file_type.is_xml()).collect())
    }

    /// Find AndroidManifest.xml files
    pub fn find_manifests(&self, root: &Path) -> Result<Vec<SourceFile>> {
        let files = self.find_files(root)?;
        Ok(files
            .into_iter()
            .filter(|f| f.file_type == FileType::XmlManifest)
            .collect())
    }

    /// Find layout XML files
    pub fn find_layouts(&self, root: &Path) -> Result<Vec<SourceFile>> {
        let files = self.find_files(root)?;
        Ok(files
            .into_iter()
            .filter(|f| f.file_type == FileType::XmlLayout)
            .collect())
    }

    /// Find navigation XML files
    pub fn find_navigation(&self, root: &Path) -> Result<Vec<SourceFile>> {
        let files = self.find_files(root)?;
        Ok(files
            .into_iter()
            .filter(|f| f.file_type == FileType::XmlNavigation)
            .collect())
    }

    /// Find menu XML files
    pub fn find_menus(&self, root: &Path) -> Result<Vec<SourceFile>> {
        let files = self.find_files(root)?;
        Ok(files
            .into_iter()
            .filter(|f| f.file_type == FileType::XmlMenu)
            .collect())
    }
}

/// Statistics about discovered files
#[derive(Debug, Default)]
pub struct FileStats {
    pub kotlin_files: usize,
    pub java_files: usize,
    pub manifest_files: usize,
    pub layout_files: usize,
    pub navigation_files: usize,
    pub menu_files: usize,
    pub other_xml_files: usize,
}

impl FileStats {
    pub fn from_files(files: &[SourceFile]) -> Self {
        let mut stats = Self::default();
        for file in files {
            match file.file_type {
                FileType::Kotlin => stats.kotlin_files += 1,
                FileType::Java => stats.java_files += 1,
                FileType::XmlManifest => stats.manifest_files += 1,
                FileType::XmlLayout => stats.layout_files += 1,
                FileType::XmlNavigation => stats.navigation_files += 1,
                FileType::XmlMenu => stats.menu_files += 1,
                FileType::XmlOther => stats.other_xml_files += 1,
            }
        }
        stats
    }

    pub fn total(&self) -> usize {
        self.kotlin_files
            + self.java_files
            + self.manifest_files
            + self.layout_files
            + self.navigation_files
            + self.menu_files
            + self.other_xml_files
    }

    pub fn source_files(&self) -> usize {
        self.kotlin_files + self.java_files
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_file_type_from_path() {
        assert_eq!(
            FileType::from_path(Path::new("src/Main.kt")),
            Some(FileType::Kotlin)
        );
        assert_eq!(
            FileType::from_path(Path::new("src/Main.java")),
            Some(FileType::Java)
        );
        assert_eq!(
            FileType::from_path(Path::new("app/src/main/AndroidManifest.xml")),
            Some(FileType::XmlManifest)
        );
        assert_eq!(
            FileType::from_path(Path::new("app/src/main/res/layout/activity_main.xml")),
            Some(FileType::XmlLayout)
        );
        assert_eq!(FileType::from_path(Path::new("README.md")), None);
    }

    #[test]
    fn test_file_type_is_source() {
        assert!(FileType::Kotlin.is_source());
        assert!(FileType::Java.is_source());
        assert!(!FileType::XmlManifest.is_source());
        assert!(!FileType::XmlLayout.is_source());
    }

    #[test]
    fn test_source_file_creation() {
        let file = SourceFile::new(PathBuf::from("test.kt"), FileType::Kotlin);
        assert_eq!(file.file_type, FileType::Kotlin);
        assert!(file.contents().is_none());
    }
}
