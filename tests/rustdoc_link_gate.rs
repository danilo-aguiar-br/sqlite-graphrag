//! GAP-SG-228: the rustdoc lints this project DENIES were never part of a gate.
//!
//! `Cargo.toml` declares `broken_intra_doc_links`, `private_intra_doc_links` and
//! `invalid_html_tags` as `deny` under `[lints.rustdoc]`. The comment there
//! reasoned that RFC 3389 maps that table onto the rustdoc command, "so
//! `cargo doc` and `cargo test --doc` now fail on their own". Half of that is
//! true.
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
//! and its doc comment linked `crate::cli::Cli::install_write_policy`, declared
//! without `pub`. GAP-SG-206's correction violated the gate GAP-SG-211 had just
//! installed, and nothing could say so.
//!
//! The scanner lives in `rustdoc_link_scan`, which explains its own algorithm
//! and its stated non-goals. This file is the gate: what is asserted about the
//! tree, and the fixtures that prove the scanner would still catch the defect it
//! was written for.

#[path = "rustdoc_link_scan/mod.rs"]
mod scan;

use scan::strict::{
    broken_links, code_identifiers, fence_marker, html_tags, prose_lines, stray_html_tags,
};
use scan::{
    analyze, doc_blocks, index_declarations, library_sources, links_in_block, parse_item, resolve,
    Carrier, Decl, DocBlock, Verdict, UNRESOLVED,
};
use std::collections::{BTreeMap, BTreeSet};

#[test]
fn no_public_doc_comment_links_to_a_private_item() {
    let sources = library_sources();
    let (findings, _) = analyze(&sources);
    if findings.is_empty() {
        return;
    }
    let mut report = String::new();
    for finding in &findings {
        report.push_str(&format!(
            "\n  {}:{}\n    link    [`{}`]\n    target  {}:{}  visibility {}\n    carrier {}:{}  {}\n",
            finding.file,
            finding.line,
            finding.link,
            finding.target_file,
            finding.target_line,
            if finding.target_vis.is_empty() {
                "(none)"
            } else {
                &finding.target_vis
            },
            finding.file,
            finding.carrier_line,
            finding.carrier,
        ));
    }
    panic!(
        "{} public doc comment(s) link to a private item. `cargo doc --no-deps` \
         exits 101 on these, and `cargo test` / `cargo clippy -- -D warnings` do \
         not see them — that blindness is why this gate exists.\n{report}\n\
         Fix by ONE of:\n  \
         a) drop the brackets and write the path as a plain code span, when the \
         target is an internal hook with no public page to point at;\n  \
         b) make the target `pub`, if it really belongs to the public surface.\n\
         `pub(crate)` does NOT fix this: rustdoc treats it as private.",
        findings.len()
    );
}

#[test]
fn every_qualified_link_resolves_or_is_declared_unreachable() {
    let sources = library_sources();
    let (_, unresolved) = analyze(&sources);
    let allowed: BTreeSet<&str> = UNRESOLVED.iter().map(|(link, _)| *link).collect();
    let orphans: Vec<String> = unresolved
        .iter()
        .filter(|(_, link)| !allowed.contains(link.as_str()))
        .map(|(file, link)| format!("{file}: {link}"))
        .collect();
    assert!(
        orphans.is_empty(),
        "the declaration index cannot reach {} `crate::`-qualified link target(s). \
         This is the blindness guard: when the item parser stops recognising a \
         declaration form, the index empties and links stop resolving in bulk \
         rather than the gate quietly passing by not looking. Either fix the \
         parser or name the form in `UNRESOLVED` with a reason.\n{}",
        orphans.len(),
        orphans.join("\n")
    );
}

#[test]
fn the_walker_accounts_for_every_doc_line() {
    for (file, source) in library_sources() {
        let total = source
            .lines()
            .filter(|line| {
                let t = line.trim_start();
                t.starts_with("///") || t.starts_with("//!")
            })
            .count();
        let assigned: usize = doc_blocks(&source, true)
            .iter()
            .map(|block| block.lines.len())
            .sum();
        assert_eq!(
            assigned, total,
            "{file}: the block walker accounted for {assigned} of {total} doc \
             lines. A dropped or merged block makes every assertion above pass \
             by not looking at the text."
        );
    }
}

#[test]
fn the_unresolved_allowlist_stays_honest() {
    let sources = library_sources();
    let corpus: String = sources
        .iter()
        .map(|(_, text)| text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    for (link, reason) in UNRESOLVED {
        assert!(
            corpus.contains(link),
            "`{link}` is allowlisted as unreachable but no longer appears in \
             `src/`. Remove the entry."
        );
        assert!(
            reason.len() > 20,
            "`{link}` is allowlisted without naming WHICH declaration form the \
             index cannot see. Reason given: {reason:?}"
        );
    }
}

#[test]
fn no_doc_link_names_an_item_this_crate_does_not_contain() {
    let sources = library_sources();
    let broken = broken_links(&sources);
    assert!(
        broken.is_empty(),
        "{} doc link(s) end in a segment that appears nowhere in `src/` outside \
         comments. No scoping rule can resolve a name the crate does not \
         contain, so each of these is a typo that `cargo doc --no-deps` reports \
         as `broken_intra_doc_links` and `cargo test` never mentions.\n{}",
        broken.len(),
        broken
            .iter()
            .map(|b| format!(
                "  {}:{}  [`{}`]  — `{}` is not declared or used anywhere",
                b.file, b.line, b.link, b.leaf
            ))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn no_doc_comment_leaves_a_raw_html_tag_in_prose() {
    let sources = library_sources();
    let stray = stray_html_tags(&sources);
    assert!(
        stray.is_empty(),
        "{} raw HTML tag(s) sit in doc prose. rustdoc parses doc comments as \
         Markdown, so `Vec<String>` written outside a code span becomes an \
         unclosed HTML tag and `invalid_html_tags` denies it. Wrap the type in \
         backticks.\n{}",
        stray.len(),
        stray
            .iter()
            .map(|t| format!("  {}:{}  {}  — {}", t.file, t.line, t.tag, t.problem))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// A crate whose docs contain one of each defect and none of the look-alikes.
///
/// Both detectors report zero on the real tree, which is the correct answer and
/// also the answer a detector that looks at nothing would give. This fixture is
/// what separates the two.
const STRICT_FIXTURE: &str = "\
//! Module prose mentioning a Vec<String> outside backticks.
//! A legitimate `Vec<String>` and a `<T>` inside code spans stay quiet,
//! as does the autolink <https://example.com> and <a@example.com>.

/// Sibling of [`present`] and of [`Self::absent_everywhere`].
///
/// The line break inside a code span is load-bearing: `--flag
/// <SECONDS>` closes on the next line and is not a tag.
///
/// ```
/// let _: Vec<String> = Vec::new();
/// ```
pub fn present() {}
";

#[test]
fn the_strict_detectors_find_what_they_claim_and_nothing_else() {
    let fixture = sources(&[("src/lib.rs", STRICT_FIXTURE)]);

    // 1. The broken link fires, and names the segment that does not exist.
    let broken = broken_links(&fixture);
    assert_eq!(broken.len(), 1, "expected one broken link; got {broken:?}");
    assert_eq!(broken[0].leaf, "absent_everywhere");
    assert_eq!(broken[0].link, "Self::absent_everywhere");

    // 2. `present` is declared in the same fixture, so the identical shape of
    //    link is NOT reported. Without this the detector could be flagging every
    //    link and still pass case 1.
    assert!(
        !broken.iter().any(|b| b.leaf == "present"),
        "a link to a name the crate declares must stay silent"
    );

    // 3. The stray tag fires exactly once: the prose `Vec<String>` on line 1.
    //    The backticked twin, the `<T>`, the fenced `Vec<String>`, the two
    //    autolinks and the code span that closes on the next line must all stay
    //    silent, or the gate is unusable on this repository's real prose.
    let stray = stray_html_tags(&fixture);
    assert_eq!(stray.len(), 1, "expected one stray tag; got {stray:?}");
    assert_eq!(stray[0].tag, "<String>");
    assert_eq!(stray[0].line, 1);

    // 4. A closing tag with nothing open is the other half of the lint.
    let unopened = sources(&[("src/lib.rs", "//! Text with a </div> and no opener.\n")]);
    let stray = stray_html_tags(&unopened);
    assert_eq!(stray.len(), 1);
    assert_eq!(stray[0].tag, "</div>");

    // 5. A matched pair and a void element are valid HTML, not defects.
    let valid = sources(&[(
        "src/lib.rs",
        "//! Text with <em>emphasis</em>, a <br> and a <span/>.\n",
    )]);
    assert!(stray_html_tags(&valid).is_empty());

    // 6. The corpus reads code and ignores prose, which is what makes case 1
    //    possible: a name that only ever appears inside a doc comment is not a
    //    declaration.
    let corpus = code_identifiers(&fixture);
    assert!(corpus.contains("present"));
    assert!(!corpus.contains("absent_everywhere"));

    // 7. The span state must cross lines. Resetting it per line reports
    //    `<SECONDS>` in `src/errors.rs`, where the opening backtick is on the
    //    previous line.
    let spanning = vec![
        (1, " `--wait-job-singleton".to_string()),
        (2, " <SECONDS>` to poll until the lock drops.".to_string()),
    ];
    let prose = prose_lines(&spanning);
    assert!(
        html_tags(&prose[1].1).is_empty(),
        "a code span that opened on the previous line must still be a code span"
    );

    // 8. Fences open and close on their own marker, so a `~~~` block is not
    //    closed by a ``` line.
    assert_eq!(fence_marker("```rust"), Some('`'));
    assert_eq!(fence_marker("~~~"), Some('~'));
    assert_eq!(fence_marker("`inline`"), None);
}

/// Crate root of the fixture tree.
///
/// Present because the carrier only counts when its module reaches the public
/// documentation, so a fixture that omits the `pub mod` chain would report no
/// finding for the right reason and prove nothing.
const ROOT_MODULES: &str = "pub mod cli;\npub mod paths;\n";

/// The `cli` module's own re-exports, completing that chain.
const CLI_MODULES: &str = "pub mod commands;\npub mod globals;\n";

/// The private half of the v1.2.8 defect, verbatim in shape.
const DEFECT_TARGET: &str = "\
pub struct Cli {}
impl Cli {
    fn install_write_policy(&self) {
        let _ = self;
    }
}
";

/// The free PUBLIC homonym that lives in `src/paths.rs` and hides the defect
/// from any resolver keyed on the last path segment alone.
const DEFECT_HOMONYM: &str = "\
pub fn install_write_policy(policy: WritePolicy) {
    let _ = policy;
}
";

/// The public carrier, with the link exactly as v1.2.8 shipped it.
const DEFECT_CARRIER: &str = "\
pub enum Commands {}
impl Commands {
    /// The conjunction is not a new list: [`crate::cli::Cli::install_write_policy`]
    /// already asks precisely this question.
    pub fn persists(&self) -> bool {
        true
    }
}
";

fn sources(parts: &[(&str, &str)]) -> Vec<(String, String)> {
    parts
        .iter()
        .map(|(name, body)| ((*name).to_string(), (*body).to_string()))
        .collect()
}

#[test]
fn the_gate_detects_what_it_claims_to_detect() {
    // 1. The shipped defect fires, and names the private declaration.
    let shipped = sources(&[
        ("src/lib.rs", ROOT_MODULES),
        ("src/cli/mod.rs", CLI_MODULES),
        ("src/cli/globals.rs", DEFECT_TARGET),
        ("src/paths.rs", DEFECT_HOMONYM),
        ("src/cli/commands.rs", DEFECT_CARRIER),
    ]);
    let (findings, _) = analyze(&shipped);
    assert_eq!(
        findings.len(),
        1,
        "the gate must flag the v1.2.8 defect; got {findings:?}"
    );
    assert_eq!(findings[0].target_file, "src/cli/globals.rs");
    assert_eq!(findings[0].target_vis, "");

    // 2. The homonym is what makes case 1 non-trivial, so the owner key is
    //    asserted against a REAL index rather than an empty one. With only the
    //    free public `install_write_policy` present, a resolver keyed on the last
    //    segment alone answers `Public` and the defect ships silently — which is
    //    exactly what happened. Keyed by owner, `Cli::install_write_policy` finds
    //    no candidate and never borrows the homonym's visibility.
    let mut decls = Vec::new();
    let mut modules = BTreeSet::new();
    index_declarations(DEFECT_HOMONYM, "src/paths.rs", &mut decls, &mut modules);
    let mut homonym_only: BTreeMap<(Option<String>, String), Vec<Decl>> = BTreeMap::new();
    for decl in decls {
        homonym_only
            .entry((decl.owner.clone(), decl.name.clone()))
            .or_default()
            .push(decl);
    }
    assert_eq!(
        resolve(&homonym_only, "crate::paths::install_write_policy"),
        Verdict::Public,
        "the free public homonym must resolve on its own path"
    );
    assert_eq!(
        resolve(&homonym_only, "crate::cli::Cli::install_write_policy"),
        Verdict::Unresolved,
        "the owner segment must stop the public homonym from answering for the \
         private method; without it the gate is blind to the shipped defect"
    );

    // 3. Making the target `pub` clears it.
    let fixed = sources(&[
        ("src/lib.rs", ROOT_MODULES),
        ("src/cli/mod.rs", CLI_MODULES),
        (
            "src/cli/globals.rs",
            "pub struct Cli {}\nimpl Cli {\n    pub fn install_write_policy(&self) {}\n}\n",
        ),
        ("src/cli/commands.rs", DEFECT_CARRIER),
    ]);
    assert!(
        analyze(&fixed).0.is_empty(),
        "a bare `pub` target is the real fix and must clear the finding"
    );

    // 4. `pub(crate)` is NOT a fix. rustdoc treats it as private, and this is the
    //    most tempting wrong correction.
    let crate_visible = sources(&[
        ("src/lib.rs", ROOT_MODULES),
        ("src/cli/mod.rs", CLI_MODULES),
        (
            "src/cli/globals.rs",
            "pub struct Cli {}\nimpl Cli {\n    pub(crate) fn install_write_policy(&self) {}\n}\n",
        ),
        ("src/cli/commands.rs", DEFECT_CARRIER),
    ]);
    assert_eq!(
        analyze(&crate_visible).0.len(),
        1,
        "`pub(crate)` must still be reported, or the gate cures one symptom \
         instead of the class"
    );

    // 5. A private carrier is not a finding: rustdoc does not document it.
    let private_carrier = sources(&[
        ("src/lib.rs", ROOT_MODULES),
        ("src/cli/mod.rs", CLI_MODULES),
        ("src/cli/globals.rs", DEFECT_TARGET),
        (
            "src/cli/commands.rs",
            "pub enum Commands {}\nimpl Commands {\n    /// [`crate::cli::Cli::install_write_policy`]\n    fn persists(&self) -> bool { true }\n}\n",
        ),
    ]);
    assert!(analyze(&private_carrier).0.is_empty());

    // 6. `pub const fn` must index as `as_str`, never as an item named `fn`.
    let (_, keyword, name) = parse_item("    pub const fn as_str(&self) -> &str {").unwrap();
    assert_eq!((keyword, name.as_str()), ("fn", "as_str"));
    let (_, keyword, name) = parse_item("pub const MAX: usize = 8;").unwrap();
    assert_eq!((keyword, name.as_str()), ("const", "MAX"));

    // 7. Reference-style definitions must not be read as their own path.
    let block = DocBlock {
        lines: vec![
            (1, " [`resolve_projection`] does the work.".to_string()),
            (2, " [`resolve_projection`]: super::gate".to_string()),
        ],
        carrier: Carrier::Private,
    };
    assert!(
        links_in_block(&block).is_empty(),
        "a shortcut whose destination is defined in the same block is not a \
         path of its own"
    );

    // 8. A bare single identifier resolves through scope, which no textual scan
    //    can see, so it is skipped rather than guessed at.
    assert_eq!(resolve(&BTreeMap::new(), "format"), Verdict::Skipped);
    assert_eq!(resolve(&BTreeMap::new(), "Self::mutates"), Verdict::Skipped);
    assert_eq!(
        resolve(&BTreeMap::new(), "std::fmt::Debug"),
        Verdict::Skipped
    );
}

#[test]
fn enum_variants_inherit_the_enum_visibility() {
    let mut decls = Vec::new();
    let mut modules = BTreeSet::new();
    index_declarations(
        "pub enum AppError {\n    Usage { message: String },\n    Timeout,\n}\n",
        "src/errors.rs",
        &mut decls,
        &mut modules,
    );
    let usage = decls
        .iter()
        .find(|d| d.name == "Usage")
        .expect("a variant must be indexed, or every link through it goes unresolved");
    assert_eq!(usage.owner.as_deref(), Some("AppError"));
    assert!(usage.is_public);
}

/// The full rustdoc run, behind `slow-tests`.
///
/// GAP-SG-235: the static scan above closes `private_intra_doc_links`, which is
/// the lint that actually regressed here, and it closes it in milliseconds. It
/// does NOT close the other two the manifest denies. `broken_intra_doc_links`
/// needs rustdoc's own path resolver — `Self::`, relative paths, re-exports,
/// the prelude — and `invalid_html_tags` needs its Markdown parser. Rebuilding
/// either is how a gate starts disagreeing with the tool it stands in for.
///
/// So this one does not stand in for anything: it runs `cargo doc`.
///
/// # Why it is not in the hot path
///
/// `cargo doc` takes the build lock and the target directory, so invoking it
/// from inside a `cargo test` that already holds both deadlocks. The isolated
/// `CARGO_TARGET_DIR` below avoids that, and pays for it by compiling the
/// dependency graph once per directory. That is minutes on a cold cache, which
/// is exactly the cost that makes a gate stop being run — hence `slow-tests`,
/// where the project already puts work it wants available but not constant.
///
/// The directory is STABLE rather than temporary, and under `target/`, so the
/// second run reuses the first one's artefacts instead of paying again. It is
/// inside the workspace on purpose: a temp dir would be swept between runs and
/// the cost would return on every invocation.
#[cfg(feature = "slow-tests")]
#[test]
fn cargo_doc_agrees_with_the_static_scan() {
    use std::path::PathBuf;
    use std::process::Command;

    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let isolated = repo.join("target").join("rustdoc-gate");
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());

    let out = Command::new(cargo)
        .args(["doc", "--no-deps", "--all-features"])
        .current_dir(&repo)
        .env("CARGO_TARGET_DIR", &isolated)
        .output()
        .expect(
            "could not run `cargo doc`. The toolchain that runs this test is the \
             one that must document the crate; a host without it is not running \
             the toolchain `rust-toolchain.toml` pins.",
        );

    if out.status.success() {
        return;
    }

    panic!(
        "`cargo doc --no-deps` failed. It enforces all three lints \
         `[lints.rustdoc]` denies, and it is the ONLY thing that enforces \
         `broken_intra_doc_links` and `invalid_html_tags` — the static scan in \
         this file covers `private_intra_doc_links` alone.\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
}
