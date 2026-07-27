//! Self-contained HTML report: one file, zero external assets, with a
//! filter box and sortable columns. A terminal caps out fast at 5000
//! findings; a page you can filter does not.

use crate::analysis::DeadCode;
use miette::{IntoDiagnostic, Result};
use std::path::PathBuf;

pub struct HtmlReporter {
    output_path: Option<PathBuf>,
    base_path: Option<PathBuf>,
}

impl HtmlReporter {
    pub fn new(output_path: Option<PathBuf>) -> Self {
        Self {
            output_path,
            base_path: None,
        }
    }

    pub fn with_base_path(mut self, base: Option<PathBuf>) -> Self {
        self.base_path = base;
        self
    }

    pub fn report(&self, dead_code: &[DeadCode]) -> Result<()> {
        let html = self.render(dead_code);
        if let Some(path) = &self.output_path {
            std::fs::write(path, &html).into_diagnostic()?;
            println!("HTML report written to: {}", path.display());
        } else {
            println!("{html}");
        }
        Ok(())
    }

    fn render(&self, dead_code: &[DeadCode]) -> String {
        let mut rows = String::new();
        for dc in dead_code {
            let file = match &self.base_path {
                Some(base) => dc
                    .declaration
                    .location
                    .file
                    .strip_prefix(base)
                    .unwrap_or(&dc.declaration.location.file)
                    .display()
                    .to_string(),
                None => dc.declaration.location.file.display().to_string(),
            };
            rows.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>\n",
                escape(dc.issue.code()),
                escape(&dc.declaration.name),
                escape(&file),
                dc.declaration.location.line,
                dc.confidence.as_str(),
                dc.risk,
                escape(&dc.message),
            ));
        }

        format!(
            r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>SearchDeadCode report — {count} finding(s)</title>
<style>
body {{ font-family: -apple-system, system-ui, sans-serif; margin: 2rem; }}
h1 {{ font-size: 1.2rem; }}
input {{ padding: 0.4rem; width: 24rem; margin-bottom: 1rem; }}
table {{ border-collapse: collapse; width: 100%; font-size: 0.85rem; }}
th, td {{ text-align: left; padding: 0.3rem 0.6rem; border-bottom: 1px solid #ddd; }}
th {{ cursor: pointer; background: #f5f5f5; position: sticky; top: 0; }}
tr:hover {{ background: #fafafa; }}
</style>
</head>
<body>
<h1>SearchDeadCode — {count} finding(s)</h1>
<input id="filter" type="search" placeholder="filter by anything: code, symbol, file…" autofocus>
<table id="findings">
<thead><tr>
<th>Code</th><th>Symbol</th><th>File</th><th>Line</th><th>Confidence</th><th>Risk</th><th>Message</th>
</tr></thead>
<tbody>
{rows}</tbody>
</table>
<script>
const input = document.getElementById('filter');
const tbody = document.querySelector('#findings tbody');
input.addEventListener('input', () => {{
  const needle = input.value.toLowerCase();
  for (const row of tbody.rows) {{
    row.style.display = row.textContent.toLowerCase().includes(needle) ? '' : 'none';
  }}
}});
document.querySelectorAll('#findings th').forEach((th, col) => {{
  let asc = true;
  th.addEventListener('click', () => {{
    const rows = Array.from(tbody.rows);
    rows.sort((a, b) => {{
      const x = a.cells[col].textContent, y = b.cells[col].textContent;
      const nx = parseFloat(x), ny = parseFloat(y);
      const cmp = !isNaN(nx) && !isNaN(ny) ? nx - ny : x.localeCompare(y);
      return asc ? cmp : -cmp;
    }});
    asc = !asc;
    rows.forEach(r => tbody.appendChild(r));
  }});
}});
</script>
</body>
</html>
"##,
            count = dead_code.len(),
            rows = rows,
        )
    }
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_entities_are_escaped() {
        assert_eq!(escape("a<b>&\"c\""), "a&lt;b&gt;&amp;&quot;c&quot;");
    }
}
