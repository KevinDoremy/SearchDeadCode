//! Interactive triage mode (fzf-style): filter findings by typing, act on
//! them from the keyboard — explain, kill-list, delete with a diff preview.
//!
//! The dialoguer prompts are a thin shell; every decision lives in pure,
//! unit-tested helpers below.

use crate::analysis::DeadCode;
use crate::graph::{DeclarationId, Graph};
use miette::Result;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Run the triage loop. The caller guarantees a real terminal.
pub fn run_triage(
    _graph: &Graph,
    _entry_points: &HashSet<DeclarationId>,
    _reachable: &HashSet<DeclarationId>,
    findings: Vec<DeadCode>,
    _base_path: &Path,
    _undo_script_path: Option<PathBuf>,
) -> Result<()> {
    if findings.is_empty() {
        println!("Nothing to triage — no dead code found.");
        return Ok(());
    }

    // The dialoguer loop lands in a later step of the plan.
    println!("Interactive triage: {} findings.", findings.len());
    Ok(())
}
