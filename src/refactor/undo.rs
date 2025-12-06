use miette::{IntoDiagnostic, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Generates an undo script to restore deleted code
pub struct UndoScript {
    /// Original file contents before deletion
    file_states: HashMap<PathBuf, String>,
}

impl UndoScript {
    pub fn new() -> Self {
        Self {
            file_states: HashMap::new(),
        }
    }

    /// Record the state of a file before modification
    pub fn record_file_state(&mut self, path: &Path, contents: &str) {
        if !self.file_states.contains_key(path) {
            self.file_states
                .insert(path.to_path_buf(), contents.to_string());
        }
    }

    /// Write the undo script to a file
    pub fn write(&self, path: &Path) -> Result<()> {
        let mut script = String::new();

        script.push_str("#!/bin/bash\n");
        script.push_str("# SearchDeadCode Undo Script\n");
        script.push_str("# Generated automatically - run to restore deleted code\n");
        script.push('\n');
        script.push_str("set -e\n");
        script.push('\n');
        script.push_str("echo 'Restoring deleted code...'\n");
        script.push('\n');

        for (file_path, contents) in &self.file_states {
            // Use heredoc to restore file contents
            let escaped_path = file_path.display().to_string().replace("'", "'\\''");
            let escaped_contents = contents.replace("'", "'\\''");

            script.push_str(&format!("# Restore {}\n", file_path.display()));
            script.push_str(&format!(
                "cat > '{}' << 'SEARCHDEADCODE_EOF'\n",
                escaped_path
            ));
            script.push_str(&escaped_contents);
            if !escaped_contents.ends_with('\n') {
                script.push('\n');
            }
            script.push_str("SEARCHDEADCODE_EOF\n");
            script.push_str(&format!("echo '  Restored: {}'\n", file_path.display()));
            script.push('\n');
        }

        script.push_str("echo 'Done! All files restored.'\n");

        std::fs::write(path, &script).into_diagnostic()?;

        // Make executable on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(path).into_diagnostic()?.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(path, perms).into_diagnostic()?;
        }

        Ok(())
    }

    /// Get the number of files recorded
    pub fn file_count(&self) -> usize {
        self.file_states.len()
    }
}

impl Default for UndoScript {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_undo_script_creation() {
        let mut script = UndoScript::new();
        script.record_file_state(Path::new("test.kt"), "class Test {}");

        assert_eq!(script.file_count(), 1);
    }

    #[test]
    fn test_undo_script_write() {
        let temp_dir = TempDir::new().unwrap();
        let script_path = temp_dir.path().join("restore.sh");

        let mut script = UndoScript::new();
        script.record_file_state(Path::new("test.kt"), "class Test {}");

        script.write(&script_path).unwrap();

        assert!(script_path.exists());
        let contents = std::fs::read_to_string(&script_path).unwrap();
        assert!(contents.contains("#!/bin/bash"));
        assert!(contents.contains("class Test {}"));
    }
}
