//! Watch mode for SearchDeadCode
//!
//! This module provides functionality for continuously monitoring
//! file changes and re-running analysis automatically.

#![allow(dead_code)] // Builder pattern methods for future configuration

use colored::Colorize;
use notify::RecursiveMode;
use notify_debouncer_mini::{new_debouncer, DebouncedEventKind};
use std::path::Path;
use std::sync::mpsc::channel;
use std::time::Duration;
use thiserror::Error;

/// Watch mode errors
#[derive(Error, Debug)]
pub enum WatchError {
    #[error("Failed to create file watcher: {0}")]
    WatcherError(#[from] notify::Error),
    #[error("Failed to receive events: {0}")]
    RecvError(#[from] std::sync::mpsc::RecvError),
}

/// File watcher for continuous analysis
pub struct FileWatcher {
    /// Debounce duration in milliseconds
    debounce_ms: u64,
    /// File extensions to watch
    extensions: Vec<String>,
}

impl FileWatcher {
    /// Create a new file watcher with default settings
    pub fn new() -> Self {
        Self {
            debounce_ms: 500,
            extensions: vec!["kt".to_string(), "java".to_string(), "xml".to_string()],
        }
    }

    /// Set debounce duration
    pub fn with_debounce_ms(mut self, ms: u64) -> Self {
        self.debounce_ms = ms;
        self
    }

    /// Set file extensions to watch
    pub fn with_extensions(mut self, extensions: Vec<String>) -> Self {
        self.extensions = extensions;
        self
    }

    /// Check if a path should trigger a rebuild
    fn should_trigger(&self, path: &Path) -> bool {
        // Check file extension
        if let Some(ext) = path.extension() {
            let ext_str = ext.to_string_lossy().to_lowercase();
            if self.extensions.iter().any(|e| e == &ext_str) {
                // Exclude build directories and hidden files
                let path_str = path.to_string_lossy();
                if path_str.contains("/build/")
                    || path_str.contains("/.gradle/")
                    || path_str.contains("/.idea/")
                    || path_str.contains("/generated/")
                {
                    return false;
                }
                return true;
            }
        }
        false
    }

    /// Start watching a directory and call the callback on changes
    pub fn watch<F>(&self, path: &Path, mut on_change: F) -> Result<(), WatchError>
    where
        F: FnMut() -> bool, // Returns false to stop watching
    {
        let (tx, rx) = channel();

        // Create debounced watcher
        let mut debouncer = new_debouncer(Duration::from_millis(self.debounce_ms), tx)?;

        // Start watching
        debouncer.watcher().watch(path, RecursiveMode::Recursive)?;

        println!();
        println!(
            "{}",
            "👁  Watch mode active. Press Ctrl+C to stop.".cyan().bold()
        );
        println!("{}", format!("   Watching: {}", path.display()).dimmed());
        println!();

        // Run initial analysis
        if !on_change() {
            return Ok(());
        }

        // Event loop
        loop {
            match rx.recv() {
                Ok(result) => {
                    match result {
                        Ok(events) => {
                            // Filter to only relevant file changes
                            let relevant: Vec<_> = events
                                .iter()
                                .filter(|e| {
                                    matches!(
                                        e.kind,
                                        DebouncedEventKind::Any | DebouncedEventKind::AnyContinuous
                                    ) && self.should_trigger(&e.path)
                                })
                                .collect();

                            if !relevant.is_empty() {
                                println!();
                                println!(
                                    "{}",
                                    format!(
                                        "🔄 Changes detected in {} file(s), re-analyzing...",
                                        relevant.len()
                                    )
                                    .yellow()
                                );

                                // List changed files (up to 5)
                                for event in relevant.iter().take(5) {
                                    if let Some(name) = event.path.file_name() {
                                        println!("   • {}", name.to_string_lossy().dimmed());
                                    }
                                }
                                if relevant.len() > 5 {
                                    println!("   • ... and {} more", relevant.len() - 5);
                                }
                                println!();

                                if !on_change() {
                                    break;
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("{}: {:?}", "Watch error".red(), e);
                        }
                    }
                }
                Err(e) => {
                    return Err(WatchError::RecvError(e));
                }
            }
        }

        Ok(())
    }
}

impl Default for FileWatcher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_should_trigger() {
        let watcher = FileWatcher::new();

        assert!(watcher.should_trigger(&PathBuf::from("src/main.kt")));
        assert!(watcher.should_trigger(&PathBuf::from("src/Main.java")));
        assert!(watcher.should_trigger(&PathBuf::from("res/layout/activity.xml")));

        assert!(!watcher.should_trigger(&PathBuf::from("src/main.rs")));
        // Use full paths with /build/ pattern
        assert!(!watcher.should_trigger(&PathBuf::from("app/build/main.kt")));
        assert!(!watcher.should_trigger(&PathBuf::from("project/.gradle/cache.kt")));
    }
}
