// Refactoring utilities - reserved for future auto-fix features
#![allow(dead_code)]
#![allow(unused_imports)]

mod editor;
mod safe_delete;
mod undo;

pub use editor::FileEditor;
pub use safe_delete::SafeDeleter;
pub use undo::UndoScript;
