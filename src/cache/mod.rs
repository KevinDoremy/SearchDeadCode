//! Incremental analysis cache for SearchDeadCode
//!
//! This module provides caching of parsed AST data and analysis results
//! to avoid re-parsing unchanged files.

#![allow(dead_code)] // Cache infrastructure for future incremental analysis

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use thiserror::Error;

/// Cache errors
#[derive(Error, Debug)]
pub enum CacheError {
    #[error("Failed to read cache file: {0}")]
    ReadError(#[from] std::io::Error),
    #[error("Failed to parse cache: {0}")]
    ParseError(#[from] serde_json::Error),
    #[error("Cache version mismatch")]
    VersionMismatch,
}

/// Current cache format version
const CACHE_VERSION: u32 = 1;

/// File metadata for change detection
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FileMetadata {
    /// File modification time (as seconds since UNIX epoch)
    pub mtime: u64,
    /// File size in bytes
    pub size: u64,
    /// Content hash (SHA-256, first 16 bytes as hex)
    pub content_hash: String,
}

impl FileMetadata {
    /// Create metadata from a file path
    pub fn from_path(path: &Path) -> std::io::Result<Self> {
        let metadata = fs::metadata(path)?;
        let mtime = metadata
            .modified()?
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let size = metadata.len();

        // Read file content and compute hash
        let content = fs::read(path)?;
        let hash = Self::compute_hash(&content);

        Ok(Self {
            mtime,
            size,
            content_hash: hash,
        })
    }

    /// Quick check if file might have changed (fast path)
    pub fn quick_changed(&self, path: &Path) -> bool {
        if let Ok(metadata) = fs::metadata(path) {
            let mtime = metadata
                .modified()
                .ok()
                .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let size = metadata.len();

            // If mtime and size match, file probably hasn't changed
            mtime != self.mtime || size != self.size
        } else {
            true // File doesn't exist, consider changed
        }
    }

    /// Full check with content hash (slow path, only if quick check fails)
    pub fn content_changed(&self, path: &Path) -> bool {
        if let Ok(content) = fs::read(path) {
            let hash = Self::compute_hash(&content);
            hash != self.content_hash
        } else {
            true
        }
    }

    fn compute_hash(content: &[u8]) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }
}

/// Cached data for a single file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileCacheEntry {
    /// File metadata for change detection
    pub metadata: FileMetadata,
    /// Declarations found in this file
    pub declarations: Vec<CachedDeclaration>,
    /// Unresolved references from this file
    pub unresolved_references: Vec<CachedReference>,
}

/// Simplified declaration for caching
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedDeclaration {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub line: usize,
    pub column: usize,
    pub fully_qualified_name: Option<String>,
    pub parent_id: Option<String>,
    pub annotations: Vec<String>,
    pub modifiers: Vec<String>,
    pub visibility: String,
    pub language: String,
}

/// Simplified reference for caching
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedReference {
    pub from_id: String,
    pub target_name: String,
    pub kind: String,
    pub line: usize,
}

/// The complete cache structure
#[derive(Debug, Serialize, Deserialize)]
pub struct AnalysisCache {
    /// Cache format version
    pub version: u32,
    /// Project root path
    pub project_root: PathBuf,
    /// Cached file data, keyed by relative path
    pub files: HashMap<PathBuf, FileCacheEntry>,
    /// Timestamp when cache was created
    pub created_at: u64,
}

impl AnalysisCache {
    /// Create a new empty cache for a project
    pub fn new(project_root: PathBuf) -> Self {
        Self {
            version: CACHE_VERSION,
            project_root,
            files: HashMap::new(),
            created_at: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }

    /// Load cache from disk
    pub fn load(cache_path: &Path) -> Result<Self, CacheError> {
        let file = fs::File::open(cache_path)?;
        let reader = BufReader::new(file);
        let cache: Self = serde_json::from_reader(reader)?;

        if cache.version != CACHE_VERSION {
            return Err(CacheError::VersionMismatch);
        }

        Ok(cache)
    }

    /// Save cache to disk
    pub fn save(&self, cache_path: &Path) -> Result<(), CacheError> {
        // Ensure parent directory exists
        if let Some(parent) = cache_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let file = fs::File::create(cache_path)?;
        let writer = BufWriter::new(file);
        serde_json::to_writer(writer, self)?;
        Ok(())
    }

    /// Get the default cache path for a project
    pub fn default_cache_path(project_root: &Path) -> PathBuf {
        project_root.join(".searchdeadcode-cache.json")
    }

    /// Check if a file needs re-parsing
    pub fn needs_reparse(&self, file_path: &Path, project_root: &Path) -> bool {
        let relative = file_path.strip_prefix(project_root).unwrap_or(file_path);

        match self.files.get(relative) {
            Some(entry) => {
                // Quick check first
                if !entry.metadata.quick_changed(file_path) {
                    return false;
                }
                // Full content hash check
                entry.metadata.content_changed(file_path)
            }
            None => true, // Not in cache
        }
    }

    /// Get cached entry for a file
    pub fn get_entry(&self, file_path: &Path, project_root: &Path) -> Option<&FileCacheEntry> {
        let relative = file_path.strip_prefix(project_root).unwrap_or(file_path);
        self.files.get(relative)
    }

    /// Update cache entry for a file
    pub fn update_entry(&mut self, file_path: &Path, project_root: &Path, entry: FileCacheEntry) {
        let relative = file_path
            .strip_prefix(project_root)
            .unwrap_or(file_path)
            .to_path_buf();
        self.files.insert(relative, entry);
    }

    /// Remove entries for files that no longer exist
    pub fn prune_missing_files(&mut self, project_root: &Path) {
        self.files.retain(|relative_path, _| {
            let full_path = project_root.join(relative_path);
            full_path.exists()
        });
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            total_files: self.files.len(),
            total_declarations: self.files.values().map(|e| e.declarations.len()).sum(),
            total_references: self
                .files
                .values()
                .map(|e| e.unresolved_references.len())
                .sum(),
        }
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub total_files: usize,
    pub total_declarations: usize,
    pub total_references: usize,
}

impl std::fmt::Display for CacheStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} files, {} declarations, {} references cached",
            self.total_files, self.total_declarations, self.total_references
        )
    }
}

/// Incremental analyzer that uses caching
pub struct IncrementalAnalyzer {
    cache: AnalysisCache,
    cache_path: PathBuf,
    project_root: PathBuf,
}

impl IncrementalAnalyzer {
    /// Create a new incremental analyzer for a project
    pub fn new(project_root: PathBuf) -> Self {
        let cache_path = AnalysisCache::default_cache_path(&project_root);
        let cache = AnalysisCache::load(&cache_path)
            .unwrap_or_else(|_| AnalysisCache::new(project_root.clone()));

        Self {
            cache,
            cache_path,
            project_root,
        }
    }

    /// Create analyzer with custom cache path
    pub fn with_cache_path(project_root: PathBuf, cache_path: PathBuf) -> Self {
        let cache = AnalysisCache::load(&cache_path)
            .unwrap_or_else(|_| AnalysisCache::new(project_root.clone()));

        Self {
            cache,
            cache_path,
            project_root,
        }
    }

    /// Check which files need re-parsing
    pub fn get_files_to_parse<'a>(
        &self,
        all_files: &'a [PathBuf],
    ) -> (Vec<&'a PathBuf>, Vec<&'a PathBuf>) {
        let mut needs_parse = Vec::new();
        let mut cached = Vec::new();

        for file in all_files {
            if self.cache.needs_reparse(file, &self.project_root) {
                needs_parse.push(file);
            } else {
                cached.push(file);
            }
        }

        (needs_parse, cached)
    }

    /// Get cache entry for a file
    pub fn get_cached(&self, file_path: &Path) -> Option<&FileCacheEntry> {
        self.cache.get_entry(file_path, &self.project_root)
    }

    /// Update cache for a file
    pub fn update_cache(&mut self, file_path: &Path, entry: FileCacheEntry) {
        self.cache
            .update_entry(file_path, &self.project_root, entry);
    }

    /// Save cache to disk
    pub fn save(&self) -> Result<(), CacheError> {
        self.cache.save(&self.cache_path)
    }

    /// Prune missing files from cache
    pub fn prune(&mut self) {
        self.cache.prune_missing_files(&self.project_root);
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        self.cache.stats()
    }

    /// Check if cache exists and is valid
    pub fn has_valid_cache(&self) -> bool {
        !self.cache.files.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_file_metadata() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.kt");
        fs::write(&test_file, "class Test {}").unwrap();

        let metadata = FileMetadata::from_path(&test_file).unwrap();
        assert!(!metadata.quick_changed(&test_file));
        assert!(!metadata.content_changed(&test_file));
    }

    #[test]
    fn test_cache_save_load() {
        let temp_dir = TempDir::new().unwrap();
        let cache_path = temp_dir.path().join("cache.json");

        let mut cache = AnalysisCache::new(temp_dir.path().to_path_buf());
        cache.files.insert(
            PathBuf::from("test.kt"),
            FileCacheEntry {
                metadata: FileMetadata {
                    mtime: 12345,
                    size: 100,
                    content_hash: "abc123".to_string(),
                },
                declarations: vec![],
                unresolved_references: vec![],
            },
        );

        cache.save(&cache_path).unwrap();

        let loaded = AnalysisCache::load(&cache_path).unwrap();
        assert_eq!(loaded.files.len(), 1);
    }
}
