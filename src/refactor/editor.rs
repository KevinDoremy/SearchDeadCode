use miette::{IntoDiagnostic, Result};
use std::path::Path;

/// File editor for modifying source files
pub struct FileEditor;

impl FileEditor {
    pub fn new() -> Self {
        Self
    }

    /// Remove a range of bytes from a file
    pub fn remove_range(&self, path: &Path, start_byte: usize, end_byte: usize) -> Result<()> {
        let contents = std::fs::read_to_string(path).into_diagnostic()?;

        if end_byte > contents.len() || start_byte > end_byte {
            return Err(miette::miette!("Invalid byte range"));
        }

        let new_contents = format!("{}{}", &contents[..start_byte], &contents[end_byte..]);

        std::fs::write(path, new_contents).into_diagnostic()?;

        Ok(())
    }

    /// Remove lines from a file
    pub fn remove_lines(&self, path: &Path, start_line: usize, end_line: usize) -> Result<()> {
        let contents = std::fs::read_to_string(path).into_diagnostic()?;
        let lines: Vec<&str> = contents.lines().collect();

        if start_line == 0 || end_line > lines.len() || start_line > end_line {
            return Err(miette::miette!("Invalid line range"));
        }

        let new_lines: Vec<&str> = lines
            .iter()
            .enumerate()
            .filter(|(i, _)| *i + 1 < start_line || *i + 1 > end_line)
            .map(|(_, line)| *line)
            .collect();

        let new_contents = new_lines.join("\n");
        std::fs::write(path, new_contents).into_diagnostic()?;

        Ok(())
    }

    /// Replace a range of text in a file
    pub fn replace_range(
        &self,
        path: &Path,
        start_byte: usize,
        end_byte: usize,
        replacement: &str,
    ) -> Result<()> {
        let contents = std::fs::read_to_string(path).into_diagnostic()?;

        if end_byte > contents.len() || start_byte > end_byte {
            return Err(miette::miette!("Invalid byte range"));
        }

        let new_contents = format!(
            "{}{}{}",
            &contents[..start_byte],
            replacement,
            &contents[end_byte..]
        );

        std::fs::write(path, new_contents).into_diagnostic()?;

        Ok(())
    }
}

impl Default for FileEditor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_remove_range() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "Hello, World!").unwrap();

        let editor = FileEditor::new();
        editor.remove_range(file.path(), 5, 7).unwrap();

        let contents = std::fs::read_to_string(file.path()).unwrap();
        assert_eq!(contents, "HelloWorld!");
    }

    #[test]
    fn test_remove_lines() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "Line 1").unwrap();
        writeln!(file, "Line 2").unwrap();
        writeln!(file, "Line 3").unwrap();

        let editor = FileEditor::new();
        editor.remove_lines(file.path(), 2, 2).unwrap();

        let contents = std::fs::read_to_string(file.path()).unwrap();
        assert!(contents.contains("Line 1"));
        assert!(!contents.contains("Line 2"));
        assert!(contents.contains("Line 3"));
    }
}
