//! Write-Only Room DAO Detector
//!
//! Detects Room DAO interfaces where data is inserted (@Insert) but never queried (@Query).
//! This indicates tables that are written to but never read, which is a form of dead code.
//!
//! ## Detection Algorithm
//!
//! 1. Find all interfaces/classes annotated with @Dao
//! 2. For each DAO, collect:
//!    - Write operations: @Insert, @Update, @Delete, @Upsert
//!    - Read operations: @Query (that returns data), @RawQuery
//! 3. Identify DAOs with only write operations (no reads)
//! 4. For DAOs with reads, check if specific tables are never queried
//!
//! ## Examples Detected
//!
//! ```kotlin
//! @Dao
//! interface AuditLogDao {
//!     @Insert
//!     suspend fun insertLog(log: AuditLog)  // DEAD: No @Query for this table
//! }
//! ```

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use crate::analysis::{Confidence, DeadCode, DeadCodeIssue};
use crate::graph::{Declaration, DeclarationId, DeclarationKind, Graph, Language, Location};

/// Information about a DAO method
#[derive(Debug, Clone)]
pub struct DaoMethod {
    pub name: String,
    pub annotation: DaoAnnotation,
    pub entity_type: Option<String>,
    pub line: usize,
}

/// Types of DAO annotations
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DaoAnnotation {
    Insert,
    Update,
    Delete,
    Upsert,
    Query,
    RawQuery,
    Transaction,
}

impl DaoAnnotation {
    pub fn is_write(&self) -> bool {
        matches!(
            self,
            DaoAnnotation::Insert
                | DaoAnnotation::Update
                | DaoAnnotation::Delete
                | DaoAnnotation::Upsert
        )
    }

    pub fn is_read(&self) -> bool {
        matches!(self, DaoAnnotation::Query | DaoAnnotation::RawQuery)
    }
}

/// Analysis result for a single DAO
#[derive(Debug, Clone)]
pub struct DaoAnalysis {
    pub name: String,
    pub file: PathBuf,
    pub line: usize,
    pub methods: Vec<DaoMethod>,
}

impl DaoAnalysis {
    pub fn new(name: String, file: PathBuf, line: usize) -> Self {
        Self {
            name,
            file,
            line,
            methods: Vec::new(),
        }
    }

    /// Check if this DAO is write-only (has writes but no reads)
    pub fn is_write_only(&self) -> bool {
        let has_writes = self.methods.iter().any(|m| m.annotation.is_write());
        let has_reads = self.methods.iter().any(|m| m.annotation.is_read());
        has_writes && !has_reads
    }

    /// Get all write methods
    pub fn write_methods(&self) -> Vec<&DaoMethod> {
        self.methods
            .iter()
            .filter(|m| m.annotation.is_write())
            .collect()
    }

    /// Get all read methods
    pub fn read_methods(&self) -> Vec<&DaoMethod> {
        self.methods
            .iter()
            .filter(|m| m.annotation.is_read())
            .collect()
    }

    /// Get entity types that are written but never queried
    pub fn write_only_entities(&self) -> HashSet<String> {
        let written: HashSet<_> = self
            .methods
            .iter()
            .filter(|m| m.annotation.is_write())
            .filter_map(|m| m.entity_type.clone())
            .collect();

        let read: HashSet<_> = self
            .methods
            .iter()
            .filter(|m| m.annotation.is_read())
            .filter_map(|m| m.entity_type.clone())
            .collect();

        written.difference(&read).cloned().collect()
    }
}

/// Result of DAO analysis across all files
#[derive(Debug, Default)]
pub struct DaoCollectionAnalysis {
    pub daos: Vec<DaoAnalysis>,
}

impl DaoCollectionAnalysis {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get all write-only DAOs
    pub fn get_write_only_daos(&self) -> Vec<&DaoAnalysis> {
        self.daos.iter().filter(|d| d.is_write_only()).collect()
    }
}

/// Detector for write-only Room DAOs
pub struct WriteOnlyDaoDetector {
    /// Consider @Transaction methods as reads (they might read internally)
    transaction_as_read: bool,
}

impl WriteOnlyDaoDetector {
    pub fn new() -> Self {
        Self {
            transaction_as_read: false,
        }
    }

    /// Analyze source code to find DAO definitions and methods
    pub fn analyze_source(&self, source: &str, file: &std::path::Path) -> DaoCollectionAnalysis {
        let mut analysis = DaoCollectionAnalysis::new();
        let lines: Vec<&str> = source.lines().collect();
        let mut current_dao: Option<DaoAnalysis> = None;
        let mut pending_annotation: Option<(DaoAnnotation, usize)> = None;

        for (line_num, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            // Check for @Dao annotation
            if trimmed.starts_with("@Dao") {
                // Look for the interface/class name on this or next lines
                for (i, dao_line) in lines
                    .iter()
                    .enumerate()
                    .skip(line_num)
                    .take(3.min(lines.len() - line_num))
                {
                    if let Some(name) = self.extract_dao_name(dao_line) {
                        if let Some(dao) = current_dao.take() {
                            analysis.daos.push(dao);
                        }
                        current_dao = Some(DaoAnalysis::new(name, file.to_path_buf(), i + 1));
                        break;
                    }
                }
                continue;
            }

            // Check for method annotations
            if let Some(annotation) = self.parse_annotation(trimmed) {
                pending_annotation = Some((annotation, line_num + 1));
                continue;
            }

            // If we have a pending annotation, look for the method
            if let Some((annotation, ann_line)) = pending_annotation.take() {
                if let Some(method_name) = self.extract_method_name(trimmed) {
                    if let Some(ref mut dao) = current_dao {
                        let entity_type = self.extract_entity_type(trimmed);
                        dao.methods.push(DaoMethod {
                            name: method_name,
                            annotation,
                            entity_type,
                            line: ann_line,
                        });
                    }
                }
            }
        }

        // Don't forget the last DAO
        if let Some(dao) = current_dao {
            analysis.daos.push(dao);
        }

        analysis
    }

    /// Parse a line for DAO annotations
    fn parse_annotation(&self, line: &str) -> Option<DaoAnnotation> {
        if line.starts_with("@Insert") {
            Some(DaoAnnotation::Insert)
        } else if line.starts_with("@Update") {
            Some(DaoAnnotation::Update)
        } else if line.starts_with("@Delete") {
            Some(DaoAnnotation::Delete)
        } else if line.starts_with("@Upsert") {
            Some(DaoAnnotation::Upsert)
        } else if line.starts_with("@Query") {
            Some(DaoAnnotation::Query)
        } else if line.starts_with("@RawQuery") {
            Some(DaoAnnotation::RawQuery)
        } else if line.starts_with("@Transaction") && self.transaction_as_read {
            Some(DaoAnnotation::Transaction)
        } else {
            None
        }
    }

    /// Extract the DAO interface/class name
    fn extract_dao_name(&self, line: &str) -> Option<String> {
        let trimmed = line.trim();

        // Match: interface SomeDao or abstract class SomeDao
        for keyword in &["interface ", "abstract class ", "class "] {
            if let Some(idx) = trimmed.find(keyword) {
                let after = &trimmed[idx + keyword.len()..];
                let name_end = after
                    .find(|c: char| !c.is_alphanumeric() && c != '_')
                    .unwrap_or(after.len());
                let name = &after[..name_end];
                if !name.is_empty() {
                    return Some(name.to_string());
                }
            }
        }

        None
    }

    /// Extract the method name from a function declaration
    fn extract_method_name(&self, line: &str) -> Option<String> {
        let trimmed = line.trim();

        // Match: fun methodName( or suspend fun methodName(
        if let Some(idx) = trimmed.find("fun ") {
            let after = &trimmed[idx + 4..];
            let name_end = after.find('(').unwrap_or(after.len());
            let name = after[..name_end].trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }

        // Match: abstract fun methodName(
        if trimmed.contains("abstract") {
            if let Some(idx) = trimmed.find("fun ") {
                let after = &trimmed[idx + 4..];
                let name_end = after.find('(').unwrap_or(after.len());
                let name = after[..name_end].trim();
                if !name.is_empty() {
                    return Some(name.to_string());
                }
            }
        }

        None
    }

    /// Extract the entity type from a method signature
    fn extract_entity_type(&self, line: &str) -> Option<String> {
        // Look for patterns like: (user: User) or (entity: SomeEntity)
        if let Some(start) = line.find('(') {
            if let Some(end) = line.find(')') {
                let params = &line[start + 1..end];
                // Simple heuristic: last word before ) that starts with uppercase
                for part in params.split(',') {
                    if let Some(colon_idx) = part.find(':') {
                        let type_part = part[colon_idx + 1..].trim();
                        let type_name = type_part
                            .split(|c: char| !c.is_alphanumeric() && c != '_')
                            .next()
                            .unwrap_or("");
                        if !type_name.is_empty()
                            && type_name
                                .chars()
                                .next()
                                .map(|c| c.is_uppercase())
                                .unwrap_or(false)
                        {
                            return Some(type_name.to_string());
                        }
                    }
                }
            }
        }
        None
    }
}

impl Default for WriteOnlyDaoDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert analysis results to DeadCode issues
pub fn analysis_to_issues(analysis: &DaoCollectionAnalysis) -> Vec<DeadCode> {
    let mut issues = Vec::new();

    for dao in analysis.get_write_only_daos() {
        for method in dao.write_methods() {
            let decl = Declaration::new(
                DeclarationId::new(dao.file.clone(), method.line, 0),
                format!("{}::{}", dao.name, method.name),
                DeclarationKind::Method,
                Location::new(dao.file.clone(), method.line, 1, 0, 0),
                Language::Kotlin,
            );

            let entity_info = method
                .entity_type
                .as_ref()
                .map(|e| format!(" for entity '{}'", e))
                .unwrap_or_default();

            let mut dead = DeadCode::new(decl, DeadCodeIssue::WriteOnlyDao);
            dead = dead.with_message(format!(
                "DAO method '{}' writes data{} but the DAO has no @Query methods to read it",
                method.name, entity_info
            ));
            dead = dead.with_confidence(Confidence::High);
            issues.push(dead);
        }
    }

    issues
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detector_creation() {
        let detector = WriteOnlyDaoDetector::new();
        assert!(!detector.transaction_as_read);
    }

    #[test]
    fn test_parse_insert_annotation() {
        let detector = WriteOnlyDaoDetector::new();
        assert_eq!(
            detector.parse_annotation("@Insert"),
            Some(DaoAnnotation::Insert)
        );
        assert_eq!(
            detector.parse_annotation("@Insert(onConflict = OnConflictStrategy.REPLACE)"),
            Some(DaoAnnotation::Insert)
        );
    }

    #[test]
    fn test_parse_query_annotation() {
        let detector = WriteOnlyDaoDetector::new();
        assert_eq!(
            detector.parse_annotation("@Query(\"SELECT * FROM users\")"),
            Some(DaoAnnotation::Query)
        );
    }

    #[test]
    fn test_extract_dao_name_interface() {
        let detector = WriteOnlyDaoDetector::new();
        assert_eq!(
            detector.extract_dao_name("interface UserDao {"),
            Some("UserDao".to_string())
        );
    }

    #[test]
    fn test_extract_dao_name_abstract_class() {
        let detector = WriteOnlyDaoDetector::new();
        assert_eq!(
            detector.extract_dao_name("abstract class BaseDao<T> {"),
            Some("BaseDao".to_string())
        );
    }

    #[test]
    fn test_extract_method_name() {
        let detector = WriteOnlyDaoDetector::new();
        assert_eq!(
            detector.extract_method_name("suspend fun insertUser(user: User)"),
            Some("insertUser".to_string())
        );
        assert_eq!(
            detector.extract_method_name("fun getAllUsers(): Flow<List<User>>"),
            Some("getAllUsers".to_string())
        );
    }

    #[test]
    fn test_extract_entity_type() {
        let detector = WriteOnlyDaoDetector::new();
        assert_eq!(
            detector.extract_entity_type("suspend fun insertUser(user: User)"),
            Some("User".to_string())
        );
        assert_eq!(
            detector.extract_entity_type("suspend fun insertHistory(history: ReadHistory)"),
            Some("ReadHistory".to_string())
        );
    }

    #[test]
    fn test_analyze_write_only_dao() {
        let detector = WriteOnlyDaoDetector::new();
        let source = r#"
@Dao
interface WriteOnlyDao {
    @Insert
    suspend fun insertLog(log: AuditLog)

    @Insert
    suspend fun insertLogs(logs: List<AuditLog>)
}
        "#;

        let analysis = detector.analyze_source(source, &PathBuf::from("test.kt"));
        assert_eq!(analysis.daos.len(), 1);
        assert!(analysis.daos[0].is_write_only());
        assert_eq!(analysis.daos[0].methods.len(), 2);
    }

    #[test]
    fn test_analyze_read_write_dao() {
        let detector = WriteOnlyDaoDetector::new();
        let source = r#"
@Dao
interface ReadWriteDao {
    @Insert
    suspend fun insertUser(user: User)

    @Query("SELECT * FROM users WHERE id = :id")
    suspend fun getUserById(id: Long): User?
}
        "#;

        let analysis = detector.analyze_source(source, &PathBuf::from("test.kt"));
        assert_eq!(analysis.daos.len(), 1);
        assert!(!analysis.daos[0].is_write_only());
    }

    #[test]
    fn test_get_write_only_daos() {
        let detector = WriteOnlyDaoDetector::new();
        let source = r#"
@Dao
interface WriteOnlyDao {
    @Insert
    suspend fun insertLog(log: AuditLog)
}

@Dao
interface ReadWriteDao {
    @Insert
    suspend fun insertUser(user: User)

    @Query("SELECT * FROM users")
    fun getAllUsers(): Flow<List<User>>
}
        "#;

        let analysis = detector.analyze_source(source, &PathBuf::from("test.kt"));
        let write_only = analysis.get_write_only_daos();
        assert_eq!(write_only.len(), 1);
        assert_eq!(write_only[0].name, "WriteOnlyDao");
    }

    #[test]
    fn test_dao_is_write_only() {
        let mut dao = DaoAnalysis::new("TestDao".to_string(), PathBuf::from("test.kt"), 1);

        // DAO with only writes
        dao.methods.push(DaoMethod {
            name: "insert".to_string(),
            annotation: DaoAnnotation::Insert,
            entity_type: Some("User".to_string()),
            line: 5,
        });
        assert!(dao.is_write_only());

        // Add a read
        dao.methods.push(DaoMethod {
            name: "getAll".to_string(),
            annotation: DaoAnnotation::Query,
            entity_type: None,
            line: 10,
        });
        assert!(!dao.is_write_only());
    }

    #[test]
    fn test_write_only_entities() {
        let mut dao = DaoAnalysis::new("TestDao".to_string(), PathBuf::from("test.kt"), 1);

        dao.methods.push(DaoMethod {
            name: "insertUser".to_string(),
            annotation: DaoAnnotation::Insert,
            entity_type: Some("User".to_string()),
            line: 5,
        });
        dao.methods.push(DaoMethod {
            name: "insertLog".to_string(),
            annotation: DaoAnnotation::Insert,
            entity_type: Some("AuditLog".to_string()),
            line: 10,
        });
        dao.methods.push(DaoMethod {
            name: "getAllUsers".to_string(),
            annotation: DaoAnnotation::Query,
            entity_type: Some("User".to_string()),
            line: 15,
        });

        let write_only = dao.write_only_entities();
        assert_eq!(write_only.len(), 1);
        assert!(write_only.contains("AuditLog"));
    }

    #[test]
    fn test_analysis_to_issues() {
        let mut analysis = DaoCollectionAnalysis::new();
        let mut dao = DaoAnalysis::new("WriteOnlyDao".to_string(), PathBuf::from("test.kt"), 1);
        dao.methods.push(DaoMethod {
            name: "insertLog".to_string(),
            annotation: DaoAnnotation::Insert,
            entity_type: Some("AuditLog".to_string()),
            line: 5,
        });
        analysis.daos.push(dao);

        let issues = analysis_to_issues(&analysis);
        assert_eq!(issues.len(), 1);
        assert!(issues[0].message.contains("insertLog"));
    }
}
