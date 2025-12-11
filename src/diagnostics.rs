use ariadne::{Color, Label, Report, ReportKind, Source};
use chumsky::error::Rich;
use owo_colors::OwoColorize;

use crate::lexer::LexError;
use crate::lexer::Token;

pub fn emit_lexer_error(err: &LexError, file_path: &str, code: &str) {
    Report::build(ReportKind::Error, (file_path, err.span.clone()))
        .with_message("Lexer error")
        .with_label(
            Label::new((file_path, err.span.clone()))
                .with_message(format!("Unexpected character: {}", err.invalid_text))
                .with_color(Color::Red),
        )
        .finish()
        .eprint((file_path, Source::from(code)))
        .unwrap();
}

pub fn emit_parser_error(err: &Rich<Token>, tokens: &[Token]) {
    let span = err.span();
    eprintln!(
        "{}\n    {} {:?} {} {:?} ({:?} {} {:?})\n    {}: {}",
        "Syntax error".red().bold(),
        "found: ".red().bold(),
        err.found()
            .map(|t| format!("{:?}", t))
            .unwrap_or_else(|| "end of input".to_string()),
        "at".bright_black(),
        span,
        tokens
            .get(span.start.saturating_sub(1))
            .unwrap_or(&Token::Semicolon),
        "->".bright_black(),
        tokens
            .get(span.end.saturating_sub(1))
            .unwrap_or(&Token::Semicolon),
        "expected".red().bold(),
        err.expected()
            .map(|e| format!("{e:?}"))
            .collect::<Vec<_>>()
            .join(&(" or ".bright_black().to_string())),
    );
}
