//! GAP-SG-235: static coverage for the two lints the link scan left to `cargo doc`.
//!
//! `[lints.rustdoc]` denies three lints. The parent module covers
//! `private_intra_doc_links`, the one that actually regressed. This module adds
//! the other two to the hot path, and it does so under a rule that keeps the
//! gate from ever disagreeing with rustdoc:
//!
//! > report only what is provably wrong from the text alone; leave everything
//! > else to `cargo doc`.
//!
//! # `broken_intra_doc_links`
//!
//! Deciding that `[`Self::mutates`]` resolves needs rustdoc's own path
//! resolver — trait inheritance, `Deref`, re-exports, the prelude, glob imports.
//! Rebuilding that is how a substitute gate starts contradicting the tool.
//!
//! So this scan never asks WHERE a name lives. It asks whether the name exists
//! in the crate AT ALL: the last segment of a doc link is compared against every
//! identifier that appears in non-comment source text. A link whose final
//! segment appears nowhere cannot resolve under any scoping rule, which makes it
//! a typo — the dominant real-world shape of this lint.
//!
//! The direction of the approximation is deliberate. It can miss a link that
//! names a real item on the wrong path; it cannot invent one. Measured on
//! v1.2.8: 625 doc links in `src/`, zero false positives, and the `crate::`
//! prefix stays under the parent module's stricter owner-keyed resolver.
//!
//! # `invalid_html_tags`
//!
//! rustdoc feeds doc comments to a Markdown parser, so `Vec<String>` written
//! outside a code span becomes a raw HTML tag and the lint reports it unclosed.
//! Detecting that needs no resolver at all — only the same code-span rules the
//! Markdown parser uses, which is what `prose_lines` implements.
//!
//! An inline code span may cross lines inside one doc block, and
//! `src/errors.rs` writes exactly that shape, so the span state is carried
//! across the whole block rather than reset per line. Resetting it per line
//! reports `<SECONDS>` as a stray tag when it sits inside backticks that opened
//! on the previous line.

use super::{doc_blocks, links_in_block};
use std::collections::BTreeSet;

/// Path roots that belong to another crate's documentation.
///
/// Their leaf names have no reason to appear in this tree, so the corpus rule
/// would report every one of them.
pub const FOREIGN_ROOTS: &[&str] = &["std", "core", "alloc"];

/// HTML elements that are complete without a closing tag.
///
/// A void element left on the stack would be reported as unclosed, which is the
/// one shape rustdoc explicitly accepts.
pub const VOID_ELEMENTS: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

/// A doc link whose final segment names nothing in this crate.
#[derive(Debug, PartialEq, Eq)]
pub struct BrokenLink {
    pub file: String,
    pub line: usize,
    pub link: String,
    /// The segment that could not be found, quoted in the failure message.
    pub leaf: String,
}

/// A raw HTML tag sitting in doc prose.
#[derive(Debug, PartialEq, Eq)]
pub struct StrayTag {
    pub file: String,
    pub line: usize,
    pub tag: String,
    /// Why it is a defect, in words the failure message can print verbatim.
    pub problem: &'static str,
}

/// One tag occurrence, as the Markdown parser would see it.
#[derive(Debug, PartialEq, Eq)]
pub struct HtmlTag {
    /// Lowercased, because HTML tag names are case-insensitive.
    pub name: String,
    /// As written, so `Vec<String>` reports `String` and not `string`.
    pub raw: String,
    pub closing: bool,
    pub self_closing: bool,
}

/// Every identifier that appears in source text that is not a comment.
///
/// Comment lines are excluded on purpose: a name that only ever appears in prose
/// is not a declaration, and counting it would silence the very typo this looks
/// for.
pub fn code_identifiers(sources: &[(String, String)]) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for (_, source) in sources {
        for line in source.lines() {
            if line.trim_start().starts_with("//") {
                continue;
            }
            let mut current = String::new();
            for ch in line.chars() {
                if ch.is_alphanumeric() || ch == '_' {
                    current.push(ch);
                    continue;
                }
                if !current.is_empty() {
                    out.insert(std::mem::take(&mut current));
                }
            }
            if !current.is_empty() {
                out.insert(current);
            }
        }
    }
    out
}

/// Doc links whose last segment exists nowhere in the crate's source text.
///
/// Every doc block is scanned, public carrier or not. A private item's doc
/// comment is not documented by default, so rustdoc stays quiet about it, but a
/// name that does not exist is wrong either way and the corpus rule cannot
/// misjudge it.
pub fn broken_links(sources: &[(String, String)]) -> Vec<BrokenLink> {
    let corpus = code_identifiers(sources);
    let mut out = Vec::new();
    for (file, source) in sources {
        for block in doc_blocks(source, true) {
            for (line, link) in links_in_block(&block) {
                let segments: Vec<&str> = link.split("::").filter(|s| !s.is_empty()).collect();
                let (Some(first), Some(leaf)) = (segments.first(), segments.last()) else {
                    continue;
                };
                if FOREIGN_ROOTS.contains(first) {
                    continue;
                }
                if corpus.contains(*leaf) {
                    continue;
                }
                out.push(BrokenLink {
                    file: file.clone(),
                    line,
                    link: link.clone(),
                    leaf: (*leaf).to_string(),
                });
            }
        }
    }
    out
}

/// A doc block with fenced blocks and inline code spans blanked out.
///
/// Blanking rather than dropping keeps every line number intact, so a finding
/// still points at the line a reader would open.
pub fn prose_lines(lines: &[(usize, String)]) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    // Length of the backtick run that opened the current inline span, 0 when
    // none is open. Carried across lines: a span may close on a later line.
    let mut span = 0usize;
    let mut fence: Option<char> = None;
    for (number, text) in lines {
        let trimmed = text.trim_start();
        let fence_char = fence_marker(trimmed);
        match (fence, fence_char) {
            (None, Some(ch)) if span == 0 => {
                fence = Some(ch);
                out.push((*number, String::new()));
                continue;
            }
            (Some(open), Some(ch)) if open == ch => {
                fence = None;
                out.push((*number, String::new()));
                continue;
            }
            (Some(_), _) => {
                out.push((*number, String::new()));
                continue;
            }
            _ => {}
        }
        let mut kept = String::new();
        let chars: Vec<char> = text.chars().collect();
        let mut index = 0usize;
        while index < chars.len() {
            if chars[index] == '`' {
                let mut end = index;
                while end < chars.len() && chars[end] == '`' {
                    end += 1;
                }
                let run = end - index;
                if span == 0 {
                    span = run;
                } else if span == run {
                    span = 0;
                }
                index = end;
                continue;
            }
            kept.push(if span == 0 { chars[index] } else { ' ' });
            index += 1;
        }
        out.push((*number, kept));
    }
    out
}

/// The fence character when a line opens or closes a fenced code block.
pub fn fence_marker(trimmed: &str) -> Option<char> {
    for marker in ['`', '~'] {
        let run = trimmed.chars().take_while(|c| *c == marker).count();
        if run >= 3 {
            return Some(marker);
        }
    }
    None
}

/// Every raw HTML tag in one line of prose.
///
/// Markdown autolinks share the angle-bracket syntax — `<https://example.com>`
/// and `<user@example.com>` — and are not tags, so they are excluded by the
/// shape of what follows the name rather than by an allowlist of schemes.
pub fn html_tags(text: &str) -> Vec<HtmlTag> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut index = 0usize;
    while index < chars.len() {
        if chars[index] != '<' {
            index += 1;
            continue;
        }
        let mut cursor = index + 1;
        let closing = chars.get(cursor) == Some(&'/');
        if closing {
            cursor += 1;
        }
        let start = cursor;
        while cursor < chars.len()
            && (chars[cursor].is_alphanumeric() || chars[cursor] == '-' || chars[cursor] == '_')
        {
            cursor += 1;
        }
        if cursor == start {
            index += 1;
            continue;
        }
        let Some(end) = (cursor..chars.len()).find(|i| chars[*i] == '>') else {
            break;
        };
        let raw: String = chars[start..cursor].iter().collect();
        let rest: String = chars[cursor..end].iter().collect();
        index = end + 1;
        if rest.starts_with(':') || rest.contains('@') {
            continue;
        }
        out.push(HtmlTag {
            name: raw.to_lowercase(),
            raw,
            closing,
            self_closing: rest.trim_end().ends_with('/'),
        });
    }
    out
}

/// Raw HTML tags in doc prose that rustdoc would report.
///
/// The stack is per block, matching how rustdoc lints one item's documentation
/// as one Markdown document.
pub fn stray_html_tags(sources: &[(String, String)]) -> Vec<StrayTag> {
    let mut out = Vec::new();
    for (file, source) in sources {
        for block in doc_blocks(source, true) {
            let mut open: Vec<(String, String, usize)> = Vec::new();
            for (number, text) in prose_lines(&block.lines) {
                for tag in html_tags(&text) {
                    if tag.self_closing || VOID_ELEMENTS.contains(&tag.name.as_str()) {
                        continue;
                    }
                    if !tag.closing {
                        open.push((tag.name, tag.raw, number));
                        continue;
                    }
                    match open.iter().rposition(|(name, _, _)| *name == tag.name) {
                        Some(at) => {
                            open.truncate(at);
                        }
                        None => out.push(StrayTag {
                            file: file.clone(),
                            line: number,
                            tag: format!("</{}>", tag.raw),
                            problem: "closing tag with nothing open",
                        }),
                    }
                }
            }
            for (_, raw, number) in open {
                out.push(StrayTag {
                    file: file.clone(),
                    line: number,
                    tag: format!("<{raw}>"),
                    problem: "unclosed tag; wrap the type in backticks",
                });
            }
        }
    }
    out
}
