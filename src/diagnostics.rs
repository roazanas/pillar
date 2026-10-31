use crate::source::{SourceFile, Span};
use serde::Serialize;
use unicode_width::UnicodeWidthChar;
#[derive(Clone, Debug, Serialize)]
pub struct Diagnostic {
    pub code: String,
    pub severity: String,
    pub message: String,
    pub span: Span,
}
impl Diagnostic {
    pub fn error(code: &str, message: impl Into<String>, span: Span) -> Self {
        Self {
            code: code.into(),
            severity: "error".into(),
            message: message.into(),
            span,
        }
    }
    pub fn json(&self, sources: &[SourceFile]) -> serde_json::Value {
        if let Some(s) = sources.get(self.span.file) {
            let ranges = s.segments(self.span);
            let truncated = ranges.len() > 256;
            let labels: Vec<_> = ranges
                .into_iter()
                .take(256)
                .map(|r| serde_json::json!({"start":s.position(r.start),"end":s.position(r.end)}))
                .collect();
            serde_json::json!({"schema_version":1,"code":self.code,"severity":self.severity,"message":self.message,"file":s.path.to_string_lossy(),"labels":labels,"labels_truncated":truncated})
        } else {
            serde_json::json!({"schema_version":1,"code":self.code,"severity":self.severity,"message":self.message})
        }
    }
    pub fn human(&self, sources: &[SourceFile], color: bool) -> String {
        let heading = if color {
            format!("\x1b[31merror[{}]\x1b[0m", self.code)
        } else {
            format!("error[{}]", self.code)
        };
        if let Some(s) = sources.get(self.span.file) {
            let p = s.location(self.span);
            let chars = s
                .text
                .lines()
                .nth(p.line - 1)
                .unwrap_or("")
                .chars()
                .map(|c| if c.is_control() { '�' } else { c })
                .collect::<Vec<_>>();
            let cursor = (p.column - 1).min(chars.len());
            let start = cursor.saturating_sub(80);
            let end = (start + 180).min(chars.len());
            let prefix = if start > 0 { "… " } else { "" };
            let suffix = if end < chars.len() { " …" } else { "" };
            let line = format!(
                "{prefix}{}{suffix}",
                chars[start..end].iter().collect::<String>()
            );
            let caret = prefix
                .chars()
                .map(|c| c.width().unwrap_or(0))
                .sum::<usize>()
                + chars[start..cursor]
                    .iter()
                    .map(|c| c.width().unwrap_or(0))
                    .sum::<usize>();
            format!(
                "{heading}: {}\n --> {}:{}:{}\n  |\n{:>3} | {}\n  | {}^\n",
                self.message,
                s.path.display(),
                p.line,
                p.column,
                p.line,
                line,
                " ".repeat(caret)
            )
        } else {
            format!("{heading}: {}\n", self.message)
        }
    }
}
pub type Result<T> = std::result::Result<T, Diagnostic>;
