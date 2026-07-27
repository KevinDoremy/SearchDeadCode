//! Last-author lookup per finding (--blame).
//!
//! One `git log -L` subprocess per finding, so this runs only on the
//! final filtered list and only when asked. Any git failure (no repo,
//! untracked file, shallow history) leaves the finding untouched.

use crate::analysis::DeadCode;
use std::path::Path;
use std::process::Command;

/// Dead but touched within this window: someone may be resurrecting it.
const RESURRECTION_WINDOW_DAYS: i64 = 30;

pub fn annotate(dead_code: &mut [DeadCode], root: &Path) {
    let today = days_since_epoch_now();
    for dc in dead_code.iter_mut() {
        let line = dc.declaration.location.line.max(1);
        let file = &dc.declaration.location.file;
        let Some((owner, date)) = last_author(root, file, line) else {
            continue;
        };
        dc.message
            .push_str(&format!(" [last touched by {owner} on {date}]"));
        if let (Some(today), Some(touched)) = (today, days_since_epoch(&date)) {
            if today - touched <= RESURRECTION_WINDOW_DAYS {
                dc.message
                    .push_str(" (recently modified — hold off deleting)");
            }
        }
    }
}

fn days_since_epoch_now() -> Option<i64> {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();
    Some((secs / 86_400) as i64)
}

/// Days from 1970-01-01 for a YYYY-MM-DD date (Howard Hinnant's civil
/// calendar algorithm).
fn days_since_epoch(date: &str) -> Option<i64> {
    let mut parts = date.splitn(3, '-');
    let y: i64 = parts.next()?.parse().ok()?;
    let m: i64 = parts.next()?.parse().ok()?;
    let d: i64 = parts.next()?.parse().ok()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    let y_adj = if m <= 2 { y - 1 } else { y };
    let era = if y_adj >= 0 { y_adj } else { y_adj - 399 } / 400;
    let yoe = y_adj - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146_097 + doe - 719_468)
}

fn last_author(root: &Path, file: &Path, line: usize) -> Option<(String, String)> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .arg("log")
        .arg("-1")
        .arg("-s")
        .arg("--format=%an|%as")
        .arg(format!("-L{line},{line}:{}", file.display()))
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_author_line(&String::from_utf8_lossy(&output.stdout))
}

fn parse_author_line(stdout: &str) -> Option<(String, String)> {
    let line = stdout.lines().find(|l| !l.trim().is_empty())?;
    let (author, date) = line.split_once('|')?;
    if author.trim().is_empty() {
        return None;
    }
    Some((author.trim().to_string(), date.trim().to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_well_formed_line_becomes_author_on_date() {
        assert_eq!(
            parse_author_line("Ghost Author|2026-07-27\n"),
            Some(("Ghost Author".to_string(), "2026-07-27".to_string()))
        );
    }

    #[test]
    fn civil_dates_convert_to_epoch_days() {
        assert_eq!(days_since_epoch("1970-01-01"), Some(0));
        assert_eq!(days_since_epoch("1970-01-02"), Some(1));
        assert_eq!(days_since_epoch("2000-03-01"), Some(11017));
        assert_eq!(days_since_epoch("not-a-date"), None);
        assert_eq!(days_since_epoch("2026-13-01"), None);
    }

    #[test]
    fn empty_output_yields_nothing() {
        assert_eq!(parse_author_line(""), None);
        assert_eq!(parse_author_line("\n\n"), None);
    }

    #[test]
    fn a_missing_separator_yields_nothing() {
        assert_eq!(parse_author_line("just some noise"), None);
    }

    #[test]
    fn an_empty_author_yields_nothing() {
        assert_eq!(parse_author_line("|2026-07-27"), None);
    }
}
