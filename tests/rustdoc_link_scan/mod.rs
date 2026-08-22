//! GAP-SG-228: the rustdoc lints this project DENIES were never part of a gate.
//!
//! `Cargo.toml` declares `broken_intra_doc_links`, `private_intra_doc_links` and
//! `invalid_html_tags` as `deny` under `[lints.rustdoc]`. The comment there
//! reasoned that RFC 3389 maps that table onto the rustdoc command, "so
//! `cargo doc` and `cargo test --doc` now fail on their own instead of depending
//! on someone remembering an environment variable". Half of that is true.
//!
//! Measured on v1.2.8: `cargo doc --no-deps` exits 101 on a real defect, while
//! `cargo test --doc` exits 0 and never mentions it. The rustdoc book explains
//! why — the lints "are only available when running rustdoc, not rustc", and
//! `rustdoc --test` extracts doctests without resolving intra-doc links. So the
//! only tool that sees this class is `cargo doc`, and `no_ci_workflows_gate`
//! states the design that leaves it out: there is no CI, `cargo test` is the
//! only automatic gate, and every other guard lives in `tests/`.
//!
//! The defect that slipped through is the third repetition of a class this
//! repository already catalogues: `Commands::persists` was born from GAP-SG-206
//! and its doc comment links `crate::cli::Cli::install_write_policy`, which is
//! declared without `pub`. GAP-SG-206's correction violated the gate GAP-SG-211
//! had just installed, and nothing could say so.
//!
//! # Why a static scan and not `cargo doc`
//!
//! `fmt_gate` can shell out because `cargo fmt` touches neither `target/` nor
//! the build lock. `cargo doc` takes both, so invoking it from inside a test
//! that `cargo test` is already running deadlocks on the same target directory.
//! Handing it an isolated `CARGO_TARGET_DIR` avoids the lock and pays for a
//! full dependency rebuild on every clean host, which is exactly the cost that
//! makes a gate stop being run.
//!
//! This scan costs milliseconds and covers `private_intra_doc_links`, the one
//! member of the trio that has actually regressed here. GAP-SG-235 added the
//! other two in [`strict`], which reports only what is decidable from the text
//! alone and leaves the rest to the `cargo doc` run that the gate file keeps
//! behind `slow-tests`.
//!
//! # Why no `syn`
//!
//! `syn` parses one file into an AST and does not resolve paths. The name index,
//! the impl-owner attribution and the resolver would still be written by hand,
//! so it would replace the cheapest part while adding `proc-macro2`, `quote` and
//! `unicode-ident` to a dev-dependency set that `c_toolchain_gate` exists to
//! keep audited. Every other gate here is a textual scanner.
//!
//! The fragility of textual scanning is answered structurally rather than
//! denied: the resolver never guesses. It either finds candidates or reports the
//! link unresolved, and an unresolved `crate::`-qualified link fails the build
//! unless it is allowlisted with a reason. Blindness surfaces red, never silent.

pub mod strict;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// `crate::`-qualified doc links the resolver cannot reach, each with its reason.
///
/// Empty on purpose, and measured: every such link in `src/` resolves today. An
/// entry here is a confession that the declaration index cannot see some form of
/// declaration, so it has to name WHICH form — otherwise the allowlist becomes
/// the place where the gate quietly stops working.
pub const UNRESOLVED: &[(&str, &str)] = &[];

/// Path prefixes the gate deliberately does not resolve.
///
/// `Self::` names an item on the impl's own type, which is public whenever the
/// impl's type is; resolving it needs trait-method inheritance and generic
/// parameters. `self::` and `super::` are relative and need the module graph.
/// The rest are foreign crates with no declaration in this tree.
///
/// Every private-link failure in this repository's history is an absolute
/// `crate::` path, so the skipped set costs no coverage of the target class.
pub const SKIPPED_ROOTS: &[&str] = &["Self", "self", "super", "std", "core", "alloc"];

/// Keywords that introduce a nameable item.
pub const ITEM_KEYWORDS: &[&str] = &[
    "fn", "struct", "enum", "trait", "type", "const", "static", "union", "mod",
];

/// Modifiers that may sit between the visibility and the item keyword.
pub const ITEM_MODIFIERS: &[&str] = &["const", "async", "unsafe", "default", "extern"];

/// A declaration the resolver can point a link at.
#[derive(Debug, Clone)]
pub struct Decl {
    /// Enclosing `impl` type or `enum`, `None` for a free item.
    pub owner: Option<String>,
    /// Item name, the last segment a link would name.
    pub name: String,
    /// `true` only for bare `pub`. See [`visibility_is_public`].
    pub is_public: bool,
    /// Visibility as written, so the failure message can quote it.
    pub vis: String,
    /// Repo-relative path.
    pub file: String,
    /// 1-based line of the declaration.
    pub line: usize,
}

/// Who carries a doc block, which decides whether the lint can fire at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Carrier {
    /// A bare-`pub` item inside a public scope: rustdoc documents it.
    Public { line: usize, decl: String },
    /// Anything else: rustdoc omits it, so a private link is not a defect.
    Private,
}

/// A run of doc lines plus the item they document.
#[derive(Debug, Clone)]
pub struct DocBlock {
    /// 1-based line number and the text after the `///` or `//!` marker.
    pub lines: Vec<(usize, String)>,
    pub carrier: Carrier,
}

/// What the resolver concluded about one link.
#[derive(Debug, PartialEq, Eq)]
pub enum Verdict {
    /// At least one candidate is bare `pub`.
    Public,
    /// Candidates exist and every one of them is private. This is the defect.
    Private {
        file: String,
        line: usize,
        vis: String,
    },
    /// No candidate at all: the index cannot see the declaration.
    Unresolved,
    /// Relative, foreign, or a bare single identifier resolved through scope.
    Skipped,
}

/// One reported defect.
#[derive(Debug)]
pub struct Finding {
    pub file: String,
    pub line: usize,
    pub link: String,
    pub carrier: String,
    pub carrier_line: usize,
    pub target_file: String,
    pub target_line: usize,
    pub target_vis: String,
}

/// `true` only for bare `pub`.
///
/// `pub(crate)`, `pub(super)` and `pub(in path)` are PRIVATE here, because that
/// is rustdoc's rule: the lint fires for anything absent from the public
/// documentation. Getting this wrong in the forgiving direction would make
/// `pub(crate)` look like a fix, and it is the most tempting wrong fix there is.
pub fn visibility_is_public(vis: &str) -> bool {
    vis == "pub"
}

/// Splits the leading visibility off a trimmed declaration line.
///
/// Returns the visibility as written and the remainder starting at the first
/// modifier or item keyword.
pub fn split_visibility(line: &str) -> (String, &str) {
    let rest = line.trim_start();
    let Some(after) = rest.strip_prefix("pub") else {
        return (String::new(), rest);
    };
    // `pubfoo` is an identifier, not a visibility.
    if after
        .chars()
        .next()
        .is_some_and(|c| c.is_alphanumeric() || c == '_')
    {
        return (String::new(), rest);
    }
    let after = after.trim_start();
    if let Some(open) = after.strip_prefix('(') {
        if let Some(close) = open.find(')') {
            let inner = &open[..close];
            return (format!("pub({inner})"), open[close + 1..].trim_start());
        }
    }
    ("pub".to_string(), after)
}

/// Extracts `(visibility, keyword, name)` from a declaration line.
///
/// The modifier loop is load-bearing. Without it `pub const fn as_str` parses as
/// an item literally named `fn`, and every link through that type stops
/// resolving — which the blindness guard would then report as dozens of
/// unresolved links rather than as silence.
pub fn parse_item(line: &str) -> Option<(String, &'static str, String)> {
    let (vis, rest) = split_visibility(line);
    let tokens: Vec<&str> = rest.split_whitespace().collect();
    let mut index = 0usize;
    while index < tokens.len() {
        let token = tokens[index];
        let is_modifier = ITEM_MODIFIERS.contains(&token);
        // `const` is a modifier only when a `fn` follows it; on its own it names
        // a constant. Same shape for the rest, which never stand alone.
        let modifier_applies = is_modifier
            && match token {
                "const" => tokens.get(index + 1).is_some_and(|next| *next == "fn"),
                "extern" => true,
                _ => true,
            };
        if modifier_applies {
            index += 1;
            // `extern "C" fn` carries an ABI string between the two.
            if token == "extern" && tokens.get(index).is_some_and(|t| t.starts_with('"')) {
                index += 1;
            }
            continue;
        }
        let keyword = ITEM_KEYWORDS.iter().find(|kw| **kw == token)?;
        let raw = tokens.get(index + 1)?;
        let name: String = raw
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        if name.is_empty() {
            return None;
        }
        return Some((vis, keyword, name));
    }
    None
}

/// Net brace movement on a line, ignoring braces inside string literals.
pub fn brace_delta(line: &str) -> i32 {
    let mut delta = 0i32;
    let mut in_string = false;
    let mut escaped = false;
    for ch in line.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' if in_string => escaped = true,
            '"' => in_string = !in_string,
            '{' if !in_string => delta += 1,
            '}' if !in_string => delta -= 1,
            _ => {}
        }
    }
    delta
}

/// The type an `impl` block applies to, with generics stripped.
///
/// Trait impls attribute to the TYPE, because that is what a link names.
pub fn impl_owner(line: &str) -> Option<String> {
    let rest = line.trim_start().strip_prefix("impl")?;
    if rest
        .chars()
        .next()
        .is_some_and(|c| c.is_alphanumeric() || c == '_')
    {
        return None;
    }
    let head = rest.split('{').next().unwrap_or(rest);
    let subject = head.rsplit(" for ").next().unwrap_or(head);
    let subject = subject.trim().trim_start_matches('<');
    let name: String = subject
        .trim_start()
        .chars()
        .skip_while(|c| !c.is_alphabetic() && *c != '_')
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    (!name.is_empty()).then_some(name)
}

/// An enum variant name on its own line inside an `enum` body.
pub fn enum_variant(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    if trimmed.starts_with("//") || trimmed.starts_with('#') {
        return None;
    }
    let name: String = trimmed
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    if name.is_empty() || !name.starts_with(|c: char| c.is_uppercase()) {
        return None;
    }
    let after = trimmed[name.len()..].trim_start();
    let opens_variant = after.is_empty()
        || after.starts_with(',')
        || after.starts_with('(')
        || after.starts_with('{')
        || after.starts_with('=');
    opens_variant.then_some(name)
}

/// Indexes every declaration in one file, keyed by `(owner, name)`.
///
/// The owner segment is what makes this work. `install_write_policy` exists
/// twice — public and free in `src/paths.rs`, private inside `impl Cli` in
/// `src/cli/globals.rs`. A resolver keyed on the last segment alone finds the
/// public one, answers `Public`, and stays silent about the shipped defect.
pub fn index_declarations(
    source: &str,
    file: &str,
    out: &mut Vec<Decl>,
    public_modules: &mut BTreeSet<String>,
) {
    let mut depth = 0i32;
    // (owner name, depth at which the block opened, is the owner public)
    let mut owners: Vec<(String, i32, bool)> = Vec::new();
    let mut enums: Vec<(String, i32, bool)> = Vec::new();

    for (offset, line) in source.lines().enumerate() {
        let number = offset + 1;
        let trimmed = line.trim_start();

        owners.retain(|(_, at, _)| depth > *at);
        enums.retain(|(_, at, _)| depth > *at);

        if !trimmed.starts_with("//") {
            if let Some(owner) = impl_owner(line) {
                let public = out
                    .iter()
                    .any(|d| d.name == owner && d.owner.is_none() && d.is_public);
                owners.push((owner, depth, public));
                depth += brace_delta(line);
                continue;
            }
            if let Some((vis, keyword, name)) = parse_item(line) {
                let is_public = visibility_is_public(&vis);
                if keyword == "mod" && is_public {
                    public_modules.insert(name.clone());
                }
                let owner = owners
                    .iter()
                    .rev()
                    .find(|(_, at, _)| depth == *at + 1)
                    .map(|(name, _, _)| name.clone());
                if keyword == "enum" {
                    enums.push((name.clone(), depth, is_public));
                }
                out.push(Decl {
                    owner,
                    name,
                    is_public,
                    vis,
                    file: file.to_string(),
                    line: number,
                });
                depth += brace_delta(line);
                continue;
            }
            // Enum variants inherit the enum's visibility; without them
            // `crate::errors::AppError::Usage` resolves to nothing.
            if let Some((enum_name, at, enum_public)) = enums.last().cloned() {
                if depth == at + 1 {
                    if let Some(variant) = enum_variant(line) {
                        out.push(Decl {
                            owner: Some(enum_name),
                            name: variant,
                            is_public: enum_public,
                            vis: if enum_public { "pub" } else { "" }.to_string(),
                            file: file.to_string(),
                            line: number,
                        });
                    }
                }
            }
        }
        depth += brace_delta(line);
    }
}

/// Strips a doc marker, returning the text after it.
pub fn doc_text(line: &str) -> Option<(bool, String)> {
    let trimmed = line.trim_start();
    if let Some(rest) = trimmed.strip_prefix("///") {
        return Some((false, rest.to_string()));
    }
    if let Some(rest) = trimmed.strip_prefix("//!") {
        return Some((true, rest.to_string()));
    }
    None
}

/// Splits a file into doc blocks and attributes each to its carrier.
///
/// Attributes legally sit between a doc comment and the item it documents, so
/// they are consumed rather than treated as the carrier.
pub fn doc_blocks(source: &str, module_is_public: bool) -> Vec<DocBlock> {
    let lines: Vec<&str> = source.lines().collect();
    let mut blocks = Vec::new();
    let mut inner: Vec<(usize, String)> = Vec::new();
    let mut index = 0usize;

    while index < lines.len() {
        let Some((is_inner, text)) = doc_text(lines[index]) else {
            index += 1;
            continue;
        };
        if is_inner {
            inner.push((index + 1, text));
            index += 1;
            continue;
        }
        let mut collected = vec![(index + 1, text)];
        let mut cursor = index + 1;
        while cursor < lines.len() {
            if let Some((false, more)) = doc_text(lines[cursor]) {
                collected.push((cursor + 1, more));
                cursor += 1;
                continue;
            }
            let trimmed = lines[cursor].trim_start();
            if trimmed.starts_with('#') || trimmed.is_empty() {
                cursor += 1;
                continue;
            }
            break;
        }
        let carrier = lines
            .get(cursor)
            .and_then(|line| parse_item(line).map(|(vis, _, _)| (vis, line)))
            .filter(|_| module_is_public)
            .and_then(|(vis, line)| {
                visibility_is_public(&vis).then(|| Carrier::Public {
                    line: cursor + 1,
                    decl: line.trim().to_string(),
                })
            })
            .unwrap_or(Carrier::Private);
        blocks.push(DocBlock {
            lines: collected,
            carrier,
        });
        index = cursor.max(index + 1);
    }

    if !inner.is_empty() {
        blocks.push(DocBlock {
            lines: inner,
            carrier: if module_is_public {
                Carrier::Public {
                    line: 1,
                    decl: "module".to_string(),
                }
            } else {
                Carrier::Private
            },
        });
    }
    blocks
}

/// The label of a reference-style link definition, e.g. `/// [`foo`]: super::bar`.
///
/// `src/agent_surface/vocabulary.rs` writes `[`resolve_projection`]` in prose and
/// defines its destination eleven lines later as `super::gate`. The real target
/// is a public module; the identically named function is private. Treating the
/// shortcut as its own path reports a defect that rustdoc does not.
pub fn reference_label(text: &str) -> Option<String> {
    let trimmed = text.trim_start();
    let rest = trimmed.strip_prefix('[')?;
    let close = rest.find(']')?;
    if !rest[close + 1..].starts_with(':') {
        return None;
    }
    Some(rest[..close].trim_matches('`').to_string())
}

/// Every intra-doc link in a block, minus the ones a definition claims.
pub fn links_in_block(block: &DocBlock) -> Vec<(usize, String)> {
    let defined: BTreeSet<String> = block
        .lines
        .iter()
        .filter_map(|(_, text)| reference_label(text))
        .collect();
    let mut out = Vec::new();
    for (number, text) in &block.lines {
        if reference_label(text).is_some() {
            continue;
        }
        for target in extract_targets(text) {
            if defined.contains(&target) {
                continue;
            }
            out.push((*number, target));
        }
    }
    out
}

/// Pulls `` [`a::B`] `` and `[text](a::B)` targets out of one doc line.
pub fn extract_targets(text: &str) -> Vec<String> {
    let bytes: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] != '[' {
            index += 1;
            continue;
        }
        let Some(close) = (index + 1..bytes.len()).find(|i| bytes[*i] == ']') else {
            break;
        };
        let inside: String = bytes[index + 1..close].iter().collect();
        let after: String = bytes[close + 1..].iter().collect();
        if let Some(rest) = after.strip_prefix('(') {
            if let Some(end) = rest.find(')') {
                let target = rest[..end].trim();
                if is_path_like(target) {
                    out.push(target.to_string());
                }
                index = close + 1;
                continue;
            }
        }
        let target = inside.trim().trim_matches('`').trim();
        if is_path_like(target) {
            out.push(target.to_string());
        }
        index = close + 1;
    }
    out
}

/// `true` when the text looks like a Rust path rather than prose.
pub fn is_path_like(text: &str) -> bool {
    !text.is_empty()
        && !text.contains(char::is_whitespace)
        && text
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == ':')
        && text
            .chars()
            .next()
            .is_some_and(|c| c.is_alphabetic() || c == '_')
}

/// Resolves one link against the declaration index.
///
/// A bare single identifier is SKIPPED unless it was `crate::`-qualified: it
/// resolves through scope, imports and the prelude, none of which a textual scan
/// can see. Including those produced the only false positive measured while
/// developing this gate.
pub fn resolve(index: &BTreeMap<(Option<String>, String), Vec<Decl>>, link: &str) -> Verdict {
    let segments: Vec<&str> = link.split("::").filter(|s| !s.is_empty()).collect();
    let Some(first) = segments.first() else {
        return Verdict::Skipped;
    };
    if SKIPPED_ROOTS.contains(first) {
        return Verdict::Skipped;
    }
    let qualified = *first == "crate";
    let path: Vec<&str> = if qualified {
        segments[1..].to_vec()
    } else {
        segments.clone()
    };
    if path.is_empty() || (!qualified && path.len() == 1) {
        return Verdict::Skipped;
    }
    let name = path[path.len() - 1].to_string();
    let owner = path.get(path.len().wrapping_sub(2)).and_then(|seg| {
        seg.starts_with(|c: char| c.is_uppercase())
            .then(|| (*seg).to_string())
    });
    let Some(candidates) = index.get(&(owner, name)) else {
        return Verdict::Unresolved;
    };
    if candidates.iter().any(|d| d.is_public) {
        return Verdict::Public;
    }
    let first = &candidates[0];
    Verdict::Private {
        file: first.file.clone(),
        line: first.line,
        vis: first.vis.clone(),
    }
}

/// Runs the whole analysis over a set of `(path, source)` pairs.
///
/// Pure over text so the self-test can drive it with the shipped defect
/// verbatim instead of trusting that the filesystem walk found it.
pub fn analyze(sources: &[(String, String)]) -> (Vec<Finding>, Vec<(String, String)>) {
    let mut decls = Vec::new();
    let mut public_modules = BTreeSet::new();
    for (file, source) in sources {
        index_declarations(source, file, &mut decls, &mut public_modules);
    }
    let mut index: BTreeMap<(Option<String>, String), Vec<Decl>> = BTreeMap::new();
    for decl in decls {
        index
            .entry((decl.owner.clone(), decl.name.clone()))
            .or_default()
            .push(decl);
    }

    let mut findings = Vec::new();
    let mut unresolved = Vec::new();
    for (file, source) in sources {
        let public = module_is_public(Path::new(file), &public_modules);
        for block in doc_blocks(source, public) {
            let Carrier::Public {
                line: carrier_line,
                ref decl,
            } = block.carrier
            else {
                continue;
            };
            for (line, link) in links_in_block(&block) {
                match resolve(&index, &link) {
                    Verdict::Private {
                        file: target_file,
                        line: target_line,
                        vis,
                    } => findings.push(Finding {
                        file: file.clone(),
                        line,
                        link,
                        carrier: decl.clone(),
                        carrier_line,
                        target_file,
                        target_line,
                        target_vis: vis,
                    }),
                    Verdict::Unresolved if link.starts_with("crate::") => {
                        unresolved.push((file.clone(), link));
                    }
                    _ => {}
                }
            }
        }
    }
    (findings, unresolved)
}

/// Approximates whether a file's module reaches the public documentation.
///
/// Name-wise, consistent with the declaration index. Under-approximating the
/// carrier is the safe direction: it can only silence a finding, never invent
/// one.
pub fn module_is_public(path: &Path, public_modules: &BTreeSet<String>) -> bool {
    let name = path.file_stem().map(|s| s.to_string_lossy().to_string());
    match name.as_deref() {
        Some("lib") | Some("main") => true,
        Some("mod") => path
            .parent()
            .and_then(Path::file_name)
            .map(|s| public_modules.contains(&s.to_string_lossy().to_string()))
            .unwrap_or(false),
        Some(stem) => public_modules.contains(stem),
        None => false,
    }
}

/// Every `.rs` file under `root`, recursively.
pub fn rust_files(root: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// Repo-relative path with forward slashes, identical on every platform.
pub fn relative(path: &Path, repo: &Path) -> String {
    path.strip_prefix(repo)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

pub fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Test-only files rustdoc never documents.
pub fn is_test_only(path: &Path) -> bool {
    let text = path.to_string_lossy().replace('\\', "/");
    text.contains("/tests/") || text.ends_with("_tests.rs") || text.ends_with("/tests.rs")
}

/// Loads `src/` as `(repo-relative path, source)` pairs.
pub fn library_sources() -> Vec<(String, String)> {
    let repo = repo_root();
    let mut files = Vec::new();
    rust_files(&repo.join("src"), &mut files);
    files.sort();
    files
        .iter()
        .filter(|path| !is_test_only(path))
        .filter_map(|path| {
            std::fs::read_to_string(path)
                .ok()
                .map(|text| (relative(path, &repo), text))
        })
        .collect()
}
