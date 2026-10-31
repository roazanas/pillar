use serde::Serialize;
use std::{ops::Range, path::PathBuf};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Span {
    pub file: usize,
    pub start: usize,
    pub end: usize,
}
impl Span {
    pub fn join(self, other: Self) -> Self {
        Self {
            end: other.end,
            ..self
        }
    }
}
#[derive(Clone, Debug)]
pub struct SourceFile {
    pub id: usize,
    pub path: PathBuf,
    pub text: String,
    pub logical: String,
    pub map: SourceMap,
    pub layout_error: Option<String>,
}
#[derive(Clone, Debug)]
pub struct SourceMap {
    cells: Vec<Option<Range<usize>>>,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Position {
    pub line: usize,
    pub column: usize,
    pub byte: usize,
}
impl SourceFile {
    pub fn vertical(id: usize, path: impl Into<PathBuf>, text: String) -> Self {
        if text.len() > 1_048_576 {
            return Self {
                id,
                path: path.into(),
                text,
                logical: String::new(),
                map: SourceMap { cells: vec![] },
                layout_error: Some("source exceeds the 1 MiB limit".into()),
            };
        }
        let mut rows = Vec::new();
        let mut offset = 0;
        for line in text.split_inclusive('\n') {
            let body = line
                .strip_suffix('\n')
                .unwrap_or(line)
                .strip_suffix('\r')
                .unwrap_or(line.strip_suffix('\n').unwrap_or(line));
            rows.push(
                body.char_indices()
                    .map(|(i, c)| (c, offset + i..offset + i + c.len_utf8()))
                    .collect::<Vec<_>>(),
            );
            offset += line.len();
        }
        let width = rows.iter().map(Vec::len).max().unwrap_or(0);
        if rows.len().checked_mul(width).is_none_or(|n| n > 4_194_304) {
            return Self {
                id,
                path: path.into(),
                text,
                logical: String::new(),
                map: SourceMap { cells: vec![] },
                layout_error: Some("source exceeds the 1 MiB / 4 million cell limit".into()),
            };
        }
        let mut logical = String::new();
        let mut cells = Vec::new();
        for col in 0..width {
            for row in &rows {
                let (c, range) = row
                    .get(col)
                    .map(|(c, r)| (*c, Some(r.clone())))
                    .unwrap_or((' ', None));
                logical.push(c);
                cells.extend(std::iter::repeat_n(range, c.len_utf8()));
            }
            logical.push('\n');
            cells.push(None);
        }
        Self {
            id,
            path: path.into(),
            text,
            logical,
            map: SourceMap { cells },
            layout_error: None,
        }
    }
    pub fn horizontal(id: usize, path: impl Into<PathBuf>, text: String) -> Self {
        if text.len() > 1_048_576 {
            return Self {
                id,
                path: path.into(),
                text,
                logical: String::new(),
                map: SourceMap { cells: vec![] },
                layout_error: Some("source exceeds the 1 MiB limit".into()),
            };
        }
        let mut cells = Vec::new();
        for (i, c) in text.char_indices() {
            cells.extend(std::iter::repeat_n(Some(i..i + c.len_utf8()), c.len_utf8()));
        }
        Self {
            id,
            path: path.into(),
            logical: text.clone(),
            text,
            map: SourceMap { cells },
            layout_error: None,
        }
    }
    pub fn span(&self, range: Range<usize>) -> Span {
        Span {
            file: self.id,
            start: range.start,
            end: range.end,
        }
    }
    pub fn segments(&self, span: Span) -> Vec<Range<usize>> {
        let mut ranges: Vec<Range<usize>> = Vec::new();
        for r in self
            .map
            .cells
            .get(span.start..span.end.min(self.map.cells.len()))
            .unwrap_or(&[])
            .iter()
            .flatten()
        {
            if ranges.last() == Some(r) {
                continue;
            }
            if let Some(last) = ranges.last_mut()
                && last.end == r.start
            {
                last.end = r.end;
                continue;
            }
            ranges.push(r.clone());
        }
        if ranges.is_empty() {
            let anchor = self
                .map
                .cells
                .get(span.start..)
                .unwrap_or(&[])
                .iter()
                .flatten()
                .next()
                .map(|r| r.start)
                .unwrap_or(self.text.len());
            ranges.push(anchor..anchor);
        }
        ranges
    }
    pub fn position(&self, byte: usize) -> Position {
        let mut byte = byte.min(self.text.len());
        while !self.text.is_char_boundary(byte) {
            byte -= 1;
        }
        let before = &self.text[..byte];
        let line = before.bytes().filter(|b| *b == b'\n').count();
        let column = before.rsplit('\n').next().unwrap_or("").chars().count();
        Position {
            line: line + 1,
            column: column + 1,
            byte,
        }
    }
    pub fn location(&self, span: Span) -> Position {
        let first = self
            .map
            .cells
            .get(span.start..span.end.min(self.map.cells.len()))
            .unwrap_or(&[])
            .iter()
            .flatten()
            .next()
            .map(|r| r.start)
            .or_else(|| {
                self.map
                    .cells
                    .get(span.start..)
                    .unwrap_or(&[])
                    .iter()
                    .flatten()
                    .next()
                    .map(|r| r.start)
            })
            .unwrap_or(self.text.len());
        self.position(first)
    }
    pub fn tab_span(&self) -> Option<Span> {
        self.logical
            .find('\t')
            .map(|start| self.span(start..start + 1))
    }
}
