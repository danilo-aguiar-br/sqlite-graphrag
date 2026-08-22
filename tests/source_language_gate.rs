//! Gate: the code under `src/` is written in English (v1.2.8, plan step 5).
//!
//! The product is multilingual, but only its OUTPUT is. Identifiers and
//! comments are read by whoever maintains the crate, and a codebase that mixes
//! two languages in its own vocabulary forces every reader to carry both.
//!
//! # What is checked, and where
//!
//! Three carriers, with deliberately different scopes:
//!
//! - IDENTIFIERS — checked everywhere under `src/`, with no exemption. A
//!   binding named `nome` is Portuguese vocabulary regardless of which module
//!   it lives in.
//! - COMMENTS — checked everywhere under `src/`, with no exemption. `src/i18n/`
//!   is where the translated STRING lives; its prose about that string is still
//!   maintenance text and belongs in English.
//! - STRINGS — exempt. A Portuguese string is the product: it is what the
//!   Portuguese user reads. The exemption is unconditional because a localised
//!   message is not confined to `src/i18n/` — `Language::Portuguese` match arms
//!   carry them at call sites too (`src/cli/globals.rs`, `src/errors.rs`), and
//!   a gate that failed there would be demanding the product stop speaking
//!   Portuguese.
//!
//! Inside a comment, spans wrapped in backticks or in double quotes are treated
//! as quoted material rather than prose: a comment that documents the
//! Portuguese half of a bilingual assertion is describing a string, not
//! speaking Portuguese.
//!
//! # Why a wordlist and not a language model
//!
//! The detector must never guess. It fires on two unambiguous signals: a
//! Latin diacritic, which English does not use in source vocabulary, and a
//! curated list of Portuguese words with no English homograph. A word that
//! exists in both languages is deliberately absent from the list, because a
//! false positive here would push a maintainer to rename correct English code.
//!
//! [`the_detector_finds_what_it_claims_to_find`] feeds the detector synthetic
//! Portuguese and synthetic English, so the gate cannot pass by being blind.

use std::path::{Path, PathBuf};

/// Portuguese words with no English homograph, lowercase and unaccented.
///
/// Unaccented spellings are listed because the diacritic check already covers
/// the accented form; these catch the ASCII-folded spellings a keyboard
/// shortcut produces.
const PORTUGUESE_WORDS: &[&str] = &[
    "nao",
    "deve",
    "devem",
    "nome",
    "nomes",
    "versao",
    "versoes",
    "inicio",
    "teto",
    "arquivo",
    "arquivos",
    "memoria",
    "memorias",
    "mensagem",
    "mensagens",
    "erro",
    "erros",
    "banco",
    "chave",
    "chaves",
    "valor",
    "valores",
    "quando",
    "porque",
    "entao",
    "assim",
    "cada",
    "todos",
    "todas",
    "sempre",
    "nunca",
    "apenas",
    "tambem",
    "mesmo",
    "outro",
    "outra",
    "primeiro",
    "ultimo",
    "vazio",
    "vazia",
    "linha",
    "linhas",
    "campo",
    "campos",
    "consulta",
    "busca",
    "grafo",
    "aresta",
    "arestas",
    "escrita",
    "leitura",
    "saida",
    "entrada",
    "usuario",
    "senha",
    "tamanho",
    "faixa",
    "limite",
    "prazo",
    "tentativa",
    "tentativas",
    "falha",
    "falhas",
    "sucesso",
    "resultado",
    "resultados",
    "processo",
    "processos",
    "caminho",
    "caminhos",
    "pasta",
    "pastas",
];

/// Latin diacritics that Portuguese uses and English source vocabulary does not.
const DIACRITICS: &[char] = &[
    'á', 'à', 'â', 'ã', 'é', 'ê', 'í', 'ó', 'ô', 'õ', 'ú', 'ü', 'ç', 'Á', 'À', 'Â', 'Ã', 'É', 'Ê',
    'Í', 'Ó', 'Ô', 'Õ', 'Ú', 'Ü', 'Ç',
];

/// Which carrier a finding sits in, so the report says what to change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Carrier {
    Identifier,
    Comment,
}

/// One offending token, with enough context to act on it.
#[derive(Debug, Clone)]
struct Finding {
    file: PathBuf,
    line: usize,
    carrier: Carrier,
    token: String,
}

/// Does this word look Portuguese?
///
/// Returns true on a diacritic or on an exact wordlist hit. Comparison is
/// lowercase so `Nome` and `NOME` are caught with the same entry.
fn looks_portuguese(word: &str) -> bool {
    if word.chars().any(|c| DIACRITICS.contains(&c)) {
        return true;
    }
    let lowered = word.to_ascii_lowercase();
    PORTUGUESE_WORDS.contains(&lowered.as_str())
}

/// Split an identifier into its vocabulary words.
///
/// `snake_case`, `SCREAMING_SNAKE` and `CamelCase` all decompose, because a
/// Portuguese word hides just as well inside `max_nome_len` as it does alone.
fn words_of_identifier(ident: &str) -> Vec<String> {
    let mut words = Vec::new();
    for chunk in ident.split('_') {
        if chunk.is_empty() {
            continue;
        }
        let mut current = String::new();
        for ch in chunk.chars() {
            if ch.is_uppercase() && !current.is_empty() {
                words.push(std::mem::take(&mut current));
            }
            current.push(ch);
        }
        if !current.is_empty() {
            words.push(current);
        }
    }
    words
}

/// Remove quoted material from a comment before reading it as prose.
///
/// Backtick spans and double-quoted spans are quotations of code or of a
/// message, not the author speaking.
///
/// `open` carries the quote state ACROSS the lines of one comment block, and
/// that is not a refinement: a wrapped citation such as
/// `("NUNCA usar string matching em / mensagens de erro")` opens on one line
/// and closes on the next, so a per-line reader would see the second half
/// unquoted and report the author as speaking Portuguese. The caller resets
/// `open` when real code appears, which ends the block.
///
/// The apostrophe is deliberately NOT an opener. English contractions would
/// open a span that never closes, and the rest of the comment would stop being
/// read — a blind gate, which is worse than a noisy one.
fn comment_prose(comment: &str, open: &mut Option<char>) -> String {
    let mut out = String::new();
    for ch in comment.chars() {
        match *open {
            Some(closer) => {
                if ch == closer {
                    *open = None;
                }
            }
            None => {
                if ch == '`' || ch == '"' {
                    *open = Some(ch);
                    out.push(' ');
                } else {
                    out.push(ch);
                }
            }
        }
    }
    out
}

/// Pull the identifier-shaped tokens out of a line of code.
fn identifiers_in(code: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    for ch in code.chars() {
        if ch.is_alphanumeric() || ch == '_' {
            current.push(ch);
        } else if !current.is_empty() {
            tokens.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

/// What the lexer is currently inside.
///
/// The state must survive a line break, which is the whole reason this is a
/// character machine and not a per-line regex. A Rust string literal may carry
/// a real newline, and a per-line reader that reopened in code mode on the next
/// line would read the second half of a Portuguese message as identifiers —
/// exactly the false positive that would have forced `src/i18n/` to be exempted
/// wholesale, blinding the gate to real defects there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Code,
    LineComment,
    BlockComment,
    Str,
    Char,
    RawStr,
}

/// Scan one source text, returning every finding it carries.
///
/// A single character pass classifies every byte as code, comment or literal,
/// then identifiers are read out of the code spans and prose out of the comment
/// spans. Literals are never read.
fn scan_source(file: &Path, source: &str) -> Vec<Finding> {
    let chars: Vec<char> = source.chars().collect();
    let mut mode = Mode::Code;
    let mut line = 1usize;
    let mut block_depth = 0usize;
    let mut raw_hashes = 0usize;
    let mut code_buf = String::new();
    let mut comment_buf = String::new();
    let mut comment_line = 1usize;
    // Quote state of the comment block currently being read. Reset when real
    // code appears, which is what ends a block.
    let mut comment_quote: Option<char> = None;
    let mut findings = Vec::new();
    let mut i = 0usize;

    // Flush helpers keep the per-line reporting honest: a buffer is attributed
    // to the line on which it OPENED, which is where a maintainer will look.
    macro_rules! flush_comment {
        () => {
            if !comment_buf.is_empty() {
                check_comment(
                    file,
                    comment_line,
                    &comment_buf,
                    &mut comment_quote,
                    &mut findings,
                );
                comment_buf.clear();
            }
        };
    }

    let mut code_line = 1usize;
    macro_rules! flush_code {
        () => {{
            if !code_buf.trim().is_empty() {
                // Only REAL code ends a comment block. Indentation alone reaches
                // here on every comment line, and resetting on it would drop the
                // quote state exactly where a wrapped citation needs it.
                comment_quote = None;
                for ident in identifiers_in(&code_buf) {
                    if words_of_identifier(&ident)
                        .iter()
                        .any(|w| looks_portuguese(w))
                    {
                        findings.push(Finding {
                            file: file.to_path_buf(),
                            line: code_line,
                            carrier: Carrier::Identifier,
                            token: ident,
                        });
                    }
                }
            }
            code_buf.clear();
        }};
    }

    while i < chars.len() {
        let ch = chars[i];
        let next = chars.get(i + 1).copied();

        if ch == '\n' {
            match mode {
                Mode::LineComment => {
                    flush_comment!();
                    mode = Mode::Code;
                }
                Mode::Code => flush_code!(),
                _ => {}
            }
            line += 1;
            code_line = line;
            i += 1;
            continue;
        }

        match mode {
            Mode::Code => {
                if ch == '/' && next == Some('/') {
                    flush_code!();
                    mode = Mode::LineComment;
                    comment_line = line;
                    i += 2;
                } else if ch == '/' && next == Some('*') {
                    flush_code!();
                    mode = Mode::BlockComment;
                    block_depth = 1;
                    comment_line = line;
                    i += 2;
                } else if ch == 'r'
                    && matches!(next, Some('"') | Some('#'))
                    && !chars
                        .get(i.wrapping_sub(1))
                        .is_some_and(|p| p.is_alphanumeric() || *p == '_')
                {
                    // `r"..."` or `r#*"..."#*`
                    let mut j = i + 1;
                    let mut hashes = 0usize;
                    while chars.get(j) == Some(&'#') {
                        hashes += 1;
                        j += 1;
                    }
                    if chars.get(j) == Some(&'"') {
                        flush_code!();
                        raw_hashes = hashes;
                        mode = Mode::RawStr;
                        i = j + 1;
                    } else {
                        code_buf.push(ch);
                        i += 1;
                    }
                } else if ch == '"' {
                    flush_code!();
                    mode = Mode::Str;
                    i += 1;
                } else if ch == '\'' {
                    // A lifetime (`'a`) is code; a char literal is not. They are
                    // told apart by the closing quote two or three chars later.
                    let is_char_literal = chars.get(i + 1) == Some(&'\\')
                        || chars.get(i + 2) == Some(&'\'')
                        || (chars.get(i + 1).is_some() && chars.get(i + 2) == Some(&'\''));
                    if is_char_literal {
                        flush_code!();
                        mode = Mode::Char;
                        i += 1;
                    } else {
                        code_buf.push(ch);
                        i += 1;
                    }
                } else {
                    code_buf.push(ch);
                    i += 1;
                }
            }
            Mode::LineComment => {
                comment_buf.push(ch);
                i += 1;
            }
            Mode::BlockComment => {
                if ch == '/' && next == Some('*') {
                    block_depth += 1;
                    i += 2;
                } else if ch == '*' && next == Some('/') {
                    block_depth -= 1;
                    i += 2;
                    if block_depth == 0 {
                        flush_comment!();
                        mode = Mode::Code;
                    }
                } else {
                    comment_buf.push(ch);
                    i += 1;
                }
            }
            Mode::Str | Mode::Char => {
                if ch == '\\' {
                    // A backslash-newline is Rust's line continuation. Skipping
                    // the pair blind would swallow that newline and leave every
                    // later finding reported one line early.
                    if chars.get(i + 1) == Some(&'\n') {
                        line += 1;
                        code_line = line;
                    }
                    i += 2;
                } else if (mode == Mode::Str && ch == '"') || (mode == Mode::Char && ch == '\'') {
                    mode = Mode::Code;
                    code_line = line;
                    i += 1;
                } else {
                    i += 1;
                }
            }
            Mode::RawStr => {
                if ch == '"' {
                    let mut j = i + 1;
                    let mut seen = 0usize;
                    while seen < raw_hashes && chars.get(j) == Some(&'#') {
                        seen += 1;
                        j += 1;
                    }
                    if seen == raw_hashes {
                        mode = Mode::Code;
                        code_line = line;
                        i = j;
                        continue;
                    }
                }
                i += 1;
            }
        }
    }
    // Code first, then the comment: the comment flush READS the quote state that
    // the code flush resets, and the reverse order leaves a dead store.
    flush_code!();
    flush_comment!();
    findings
}

/// Record every Portuguese word a comment line speaks in its own voice.
fn check_comment(
    file: &Path,
    line: usize,
    comment: &str,
    open: &mut Option<char>,
    findings: &mut Vec<Finding>,
) {
    let prose = comment_prose(comment, open);
    for word in prose.split(|c: char| !c.is_alphanumeric() && c != '_') {
        if word.is_empty() {
            continue;
        }
        if looks_portuguese(word) {
            findings.push(Finding {
                file: file.to_path_buf(),
                line,
                carrier: Carrier::Comment,
                token: word.to_string(),
            });
        }
    }
}

/// Trees whose `.rs` files this gate reads.
///
/// `tests/` joined `src/` in v1.2.8 (GAP-SG-287). It was left out when the gate
/// was installed, and the omission was a chalk line rather than an exemption:
/// the rule has always been that the code of this project is written in English,
/// and a test is code. What kept it out was cost, not principle.
const ROOTS: &[&str] = &["src", "tests"];

/// Paths this gate does not read, each with the reason written beside it.
///
/// A list of exceptions is only honest while every entry carries its argument,
/// so the meta-test below refuses an entry whose reason is empty. Matching is by
/// path prefix, which is what lets one entry cover a whole auxiliary tree.
const EXEMPT: &[(&str, &str)] = &[
    (
        "tests/source_language_gate.rs",
        "carries PORTUGUESE_WORDS and the fixtures that prove the detector \
         fires, so reading itself would report the wordlist as a violation of \
         the wordlist",
    ),
    (
        "tests/mock-llm",
        "a stub binary built as its own crate to stand in for the provider, not \
         a suite of this package",
    ),
];

/// Is `path` covered by an [`EXEMPT`] entry?
fn is_exempt(path: &Path) -> bool {
    let as_text = path.to_string_lossy().replace('\\', "/");
    EXEMPT
        .iter()
        .any(|(prefix, _)| as_text == *prefix || as_text.starts_with(&format!("{prefix}/")))
}

/// Every `.rs` file under [`ROOTS`], minus what [`EXEMPT`] excuses.
fn source_files() -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    for root in ROOTS {
        walk(Path::new(root), &mut out);
    }
    out.retain(|p| !is_exempt(p));
    out.sort();
    out
}

#[test]
fn the_source_tree_speaks_english() {
    let mut findings = Vec::new();
    for file in source_files() {
        let source = std::fs::read_to_string(&file)
            .unwrap_or_else(|e| panic!("read {}: {e}", file.display()));
        findings.extend(scan_source(&file, &source));
    }

    assert!(
        findings.is_empty(),
        "Portuguese vocabulary in source code ({} finding(s)); \
         identifiers and comments must be English, strings may stay localised:\n{}",
        findings.len(),
        findings
            .iter()
            .map(|f| format!(
                "  {}:{} [{:?}] {}",
                f.file.display(),
                f.line,
                f.carrier,
                f.token
            ))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn the_detector_finds_what_it_claims_to_find() {
    let path = Path::new("fixture.rs");

    // A Portuguese identifier, unaccented.
    let hits = scan_source(path, "let nome = row.name;\n");
    assert_eq!(hits.len(), 1, "unaccented Portuguese identifier must fire");
    assert_eq!(hits[0].carrier, Carrier::Identifier);

    // A Portuguese word buried inside a compound identifier.
    let hits = scan_source(path, "let max_nome_len = 80;\n");
    assert_eq!(
        hits.len(),
        1,
        "Portuguese inside a compound identifier must fire"
    );

    // A diacritic in a comment.
    let hits = scan_source(path, "// a distância mínima vence\n");
    assert_eq!(hits[0].carrier, Carrier::Comment, "diacritic must fire");

    // An unaccented Portuguese comment.
    let hits = scan_source(path, "// isso deve contar como backlog\n");
    assert!(
        !hits.is_empty(),
        "unaccented Portuguese prose in a comment must fire"
    );

    // English code and English prose are clean.
    let clean = "/// Returns the shortest path between two nodes.\n\
                 pub fn shortest_path(from: &str, to: &str) -> usize { 0 }\n";
    assert!(
        scan_source(path, clean).is_empty(),
        "English source must not fire: {:?}",
        scan_source(path, clean)
    );

    // A localised STRING is the product, not a violation.
    let localised = "    Language::Portuguese => \"nome deve ser kebab-case\".to_string(),\n";
    assert!(
        scan_source(path, localised).is_empty(),
        "a Portuguese string literal must be exempt: {:?}",
        scan_source(path, localised)
    );

    // Quoted material inside a comment is a quotation, not the author speaking.
    let quoted = "    // Locale-safe: EN \"invalid source\" / PT \"fonte inválida\"\n";
    assert!(
        scan_source(path, quoted).is_empty(),
        "quoted material in a comment must be exempt: {:?}",
        scan_source(path, quoted)
    );

    // A backticked code sample inside a comment is likewise a quotation.
    let backticked = "    // `config set embedding.dim nao-numero` reported success\n";
    assert!(
        scan_source(path, backticked).is_empty(),
        "backticked sample in a comment must be exempt: {:?}",
        scan_source(path, backticked)
    );
}

#[test]
fn the_scan_actually_reaches_the_source_tree() {
    // A gate that walked an empty tree would pass by finding nothing at all.
    let files = source_files();
    assert!(
        files.len() > 100,
        "expected the crate's source tree, found {} file(s)",
        files.len()
    );
}

#[test]
fn the_gate_reads_both_trees_it_declares() {
    let files = source_files();
    // A root that silently resolves to nothing turns the gate green by reading
    // no code at all, which is the failure mode a coverage widening invites.
    for root in ROOTS {
        let prefix = format!("{root}/");
        let seen = files
            .iter()
            .filter(|p| p.to_string_lossy().replace('\\', "/").starts_with(&prefix))
            .count();
        assert!(
            seen > 10,
            "root `{root}` contributed {seen} file(s); the walk is not reaching it"
        );
    }
}

#[test]
fn every_exemption_carries_a_reason_and_names_something_real() {
    for (path, reason) in EXEMPT {
        assert!(
            !reason.trim().is_empty(),
            "exemption `{path}` has no reason written beside it"
        );
        // A stale entry excuses nothing and hides that it excuses nothing, so it
        // has to fail loudly rather than sit in the list looking load-bearing.
        assert!(
            Path::new(path).exists(),
            "exemption `{path}` names a path that no longer exists"
        );
    }
}

#[test]
fn the_exemption_filter_only_matches_whole_path_segments() {
    // `tests/mock-llm` must not excuse `tests/mock-llm-adjacent.rs`.
    assert!(is_exempt(Path::new("tests/mock-llm/src/main.rs")));
    assert!(is_exempt(Path::new("tests/source_language_gate.rs")));
    assert!(!is_exempt(Path::new("tests/mock-llm-adjacent.rs")));
    assert!(!is_exempt(Path::new("tests/schema_support/mod.rs")));
}
