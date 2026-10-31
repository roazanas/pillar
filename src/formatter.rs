use crate::{
    diagnostics::{Diagnostic, Result},
    lexer::{self, Kind},
    parser,
    source::SourceFile,
    transposer,
};
/// Canonical horizontal form, preserving comment contents and token spelling.
pub fn horizontal(source: &SourceFile) -> Result<String> {
    let tokens = lexer::tokenize(source)?;
    parser::parse(&tokens, source.id, source.logical.len())?;
    let mut lines = Vec::<String>::new();
    let mut line = String::new();
    let mut indent = 0usize;
    let mut parens = 0usize;
    let mut previous: Option<Kind> = None;
    fn flush(lines: &mut Vec<String>, line: &mut String, indent: usize) {
        if !line.trim().is_empty() {
            lines.push(format!("{}{}", "  ".repeat(indent), line.trim()));
        }
        line.clear();
    }
    for token in tokens {
        let k = &token.kind;
        if matches!(k, Kind::Space) {
            continue;
        }
        if matches!(k, Kind::LineComment | Kind::BlockComment) {
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(&token.text);
            flush(&mut lines, &mut line, indent);
            previous = None;
            continue;
        }
        match k {
            Kind::LParen | Kind::LBracket => {
                if matches!(k, Kind::LParen)
                    && previous
                        .as_ref()
                        .is_some_and(|p| matches!(p, Kind::If | Kind::While))
                {
                    line.push(' ');
                }
                line.push_str(&token.text);
                parens += 1;
            }
            Kind::RParen | Kind::RBracket => {
                while line.ends_with(' ') {
                    line.pop();
                }
                line.push_str(&token.text);
                parens = parens.saturating_sub(1);
            }
            Kind::LBrace => {
                flush(&mut lines, &mut line, indent);
                lines.push(format!("{}{{", "  ".repeat(indent)));
                indent += 1;
            }
            Kind::RBrace => {
                flush(&mut lines, &mut line, indent);
                indent = indent.saturating_sub(1);
                lines.push(format!("{}}}", "  ".repeat(indent)));
            }
            Kind::End => {
                line.push('~');
                flush(&mut lines, &mut line, indent);
            }
            Kind::Comma => {
                line.push_str(", ");
            }
            Kind::Colon => {
                line.push_str(": ");
            }
            Kind::Scope | Kind::Dot => {
                while line.ends_with(' ') {
                    line.pop();
                }
                line.push_str(&token.text);
            }
            _ => {
                let no_space = previous.as_ref().is_some_and(|p| {
                    matches!(
                        p,
                        Kind::LParen | Kind::LBracket | Kind::Scope | Kind::Dot | Kind::Not
                    )
                });
                if !line.is_empty() && !line.ends_with(' ') && !no_space {
                    line.push(' ');
                }
                line.push_str(&token.text);
            }
        }
        previous = Some(token.kind);
    }
    flush(&mut lines, &mut line, indent);
    let _ = parens;
    Ok(lines.join("\n") + "\n")
}
pub fn vertical(source: &SourceFile) -> Result<String> {
    let h = horizontal(source)?;
    let out = transposer::transpose(h.lines().collect(), true)
        .into_iter()
        .map(|l| l.trim_end().to_owned())
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    let formatted = SourceFile::vertical(source.id, &source.path, out.clone());
    let before = lexer::tokenize(source)?;
    let after = lexer::tokenize(&formatted)?;
    let signature = |ts: Vec<crate::lexer::Token>| {
        ts.into_iter()
            .filter(|t| !matches!(t.kind, Kind::Space))
            .map(|t| {
                let text = if matches!(t.kind, Kind::LineComment | Kind::BlockComment) {
                    t.text
                        .lines()
                        .map(str::trim_end)
                        .collect::<Vec<_>>()
                        .join("\n")
                } else {
                    t.text
                };
                (t.kind, text)
            })
            .collect::<Vec<_>>()
    };
    if signature(before) != signature(after) {
        return Err(Diagnostic::error(
            "E0400",
            "formatting would change token or comment boundaries",
            source.span(0..0),
        ));
    }
    Ok(out)
}
