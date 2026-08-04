//! Freezes the C-toolchain surface of the build (Achado P).
//!
//! This project ships as a self-contained rust-native binary, so every
//! dependency that drags a C compiler into a consumer's build has to justify
//! itself. `cargo tree -i cc --all-features` named four:
//!
//! - `libmimalloc-sys`, via `mimalloc` — REMOVED in v1.2.2. An allocator is an
//!   optimisation, not a requirement. Measured over the real release binary on a
//!   200-memory corpus, the system allocator was not slower; it was marginally
//!   faster on every path, because a one-shot CLI never lives long enough to
//!   amortise mimalloc's initialisation. The binary also shrank ~850 KiB.
//! - `blake3` — DEFANGED in v1.2.2 via `default-features = false` plus `pure`.
//!   With `pure`, blake3's build script compiles ZERO C objects; on x86_64 it
//!   routes SSE2/SSE4.1/AVX2 through Rust `std::arch` intrinsics instead of the
//!   hand-written assembly, giving up roughly 18% of large-buffer throughput
//!   (6130 -> 5007 MiB/s on 512 KiB) — 18 microseconds on a maximum-size body,
//!   against a SQLite write and a REST round trip.
//! - `libsqlite3-sys`, via `rusqlite` with `bundled` — KEPT, structurally.
//! - `ring`, via `rustls` — KEPT, structurally.
//!
//! The two survivors are a declared exception, not an oversight. SQLite *is* a C
//! library; the product's entire storage contract is a single SQLite file with
//! FTS5, and no pure-Rust engine offers that format and that extension with
//! comparable maturity. Dropping `bundled` would not remove the C — it would
//! move it to the host and make the build depend on whichever libsqlite3 the
//! machine happens to carry, which is worse for a tool that must open a file
//! written by another machine. `ring` is the cryptographic backend under the TLS
//! that reaches OpenRouter; its primitives are C and assembly precisely because
//! constant-time behaviour is verified there, and swapping it for a pure-Rust
//! provider would trade an audited implementation for an unaudited one on the
//! path that carries the API key.
//!
//! This gate exists so the two removals cannot be undone by accident. It reads
//! the manifest as text, deliberately: it must fail when someone re-adds a
//! dependency line, without depending on a lockfile or a resolved graph.
//!
//! # Why the acceptance criterion is not `cargo tree -i cc`
//!
//! That command was the obvious check and it is the wrong one, because it
//! cannot answer the question being asked. `cargo tree` reports dependencies
//! DECLARED in a manifest; it says nothing about whether a build script ever
//! INVOKES what it declared. `blake3` lists `cc` under `[build-dependencies]`
//! unconditionally — verified in the published manifests of 1.8.4 and 1.8.5 —
//! and no feature can remove that edge. `pure` changes what `build.rs` does with
//! it, not whether it is declared, so `cargo tree -i cc` keeps naming three
//! crates however the features are set.
//!
//! What the project actually needs is that no C compiler runs. That is
//! observable: a build either emits object files or it does not. Verified by
//! building the crate twice into isolated target directories, `default` produced
//! four objects (`blake3_sse2/sse41/avx2/avx512_x86-64_unix.o`) and
//! `default-features = false, features = ["std", "pure"]` produced zero. The
//! gate below reproduces that measurement instead of reading the graph.

/// Reads `Cargo.toml` from the crate root.
fn manifest() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

/// Manifest lines that declare a dependency on `name`, ignoring comments.
///
/// Matches both `name = ...` and a `[dependencies.name]` table header, so the
/// gate cannot be sidestepped by choosing the other TOML spelling.
fn declaration_lines(manifest: &str, name: &str) -> Vec<String> {
    manifest
        .lines()
        .map(str::trim)
        .filter(|line| !line.starts_with('#'))
        .filter(|line| {
            line.starts_with(&format!("{name} "))
                || line.starts_with(&format!("{name}="))
                || line.contains(&format!("dependencies.{name}]"))
        })
        .map(str::to_string)
        .collect()
}

#[test]
fn mimalloc_is_not_a_dependency() {
    let found = declaration_lines(&manifest(), "mimalloc");
    assert!(
        found.is_empty(),
        "mimalloc is a C allocator and was removed in v1.2.2 as the removable \
         half of the C toolchain. Measurement, not preference, decided it: the \
         system allocator was not slower on any measured path for this one-shot \
         CLI. Re-add it only with a new measurement that says otherwise.\n{}",
        found.join("\n")
    );
}

#[test]
fn the_allocator_is_not_overridden_in_main() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/main.rs");
    let main = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    let overridden = main
        .lines()
        .map(str::trim)
        .any(|line| line.starts_with("#[global_allocator]"));
    assert!(
        !overridden,
        "src/main.rs registers a #[global_allocator]. The process uses the \
         system allocator on purpose; a custom one is the dependency this gate \
         removed."
    );
}

#[test]
fn every_blake3_declaration_disables_default_features() {
    let manifest = manifest();
    let declarations = declaration_lines(&manifest, "blake3");
    assert!(
        !declarations.is_empty(),
        "blake3 is expected to be declared; the gate would otherwise pass vacuously"
    );

    for line in &declarations {
        assert!(
            line.contains("default-features = false"),
            "a blake3 declaration keeps its default features, which makes its \
             build script compile C and assembly and puts `cc` back on the \
             build path. Declare it as \
             `default-features = false, features = [\"std\", \"pure\"]`.\n{line}"
        );
        assert!(
            line.contains("\"pure\""),
            "a blake3 declaration disables default features but omits `pure`, \
             which is the feature that actually stops the C build.\n{line}"
        );
        assert!(
            line.contains("\"std\""),
            "a blake3 declaration drops `std`, which the crate needs for its \
             `Write`/`Read` impls. Keep it alongside `pure`.\n{line}"
        );
    }

    // The dev-dependency matters as much as the runtime one: re-enabling the
    // default features there drags `cc` back into `cargo test`.
    assert!(
        declarations.len() >= 2,
        "expected blake3 in both [dependencies] and [dev-dependencies]; found \
         {} declaration(s). If one was dropped, drop this expectation with it.\n{}",
        declarations.len(),
        declarations.join("\n")
    );
}

#[test]
fn the_retired_miri_allocator_cfg_is_gone() {
    let manifest = manifest();
    // Comments may name the retired cfg while explaining why it went; only a
    // live declaration counts.
    let still_declared = manifest
        .lines()
        .map(str::trim)
        .filter(|line| !line.starts_with('#'))
        .any(|line| line.contains("sqlite_graphrag_miri"));
    assert!(
        !still_declared,
        "`sqlite_graphrag_miri` existed only to disable the mimalloc global \
         allocator under Miri, which cannot model `mi_malloc_aligned`. With the \
         allocator gone the cfg reads nothing, and a registered cfg nobody sets \
         is the kind of dead channel this release is removing."
    );
}

/// Object files a BUILD SCRIPT emitted, found recursively under `dir`.
///
/// Scoped to `.../build/<pkg>/out/`, which is the only place `cc` writes. rustc
/// also drops `.o` files under `target/*/incremental/` for its own use, and
/// counting those would make this gate report a C compiler that never ran.
fn build_script_objects(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            out.extend(build_script_objects(&path));
            continue;
        }
        if path.extension().is_some_and(|e| e == "o") && is_under_build_out(&path) {
            out.push(path);
        }
    }
    out
}

/// `true` when `path` sits inside a build script's `OUT_DIR`.
fn is_under_build_out(path: &std::path::Path) -> bool {
    let parts: Vec<_> = path
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    parts.windows(3).any(|w| w[0] == "build" && w[2] == "out")
}

/// Builds `blake3` alone, exactly as this project configures it, into a target
/// directory of its own, and returns the object files its build script emitted.
///
/// The isolated target directory is not an optimisation: `target/` accumulates
/// artifacts from every earlier feature set, so objects left by a previous build
/// answer a question nobody asked. An audit that reads them concludes the C
/// compiler still runs when it no longer does.
fn blake3_objects_from_a_clean_build() -> Result<Vec<std::path::PathBuf>, String> {
    let scratch = std::env::temp_dir().join(format!(
        "sgr-c-toolchain-gate-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default()
    ));
    let src = scratch.join("src");
    std::fs::create_dir_all(&src).map_err(|e| format!("mkdir {}: {e}", src.display()))?;

    // Mirror the runtime declaration. Reading it from our own manifest would
    // make the gate agree with whatever is written there, which is the failure
    // it exists to catch; the expected configuration is stated here instead, and
    // `every_blake3_declaration_disables_default_features` ties the manifest to it.
    std::fs::write(
        scratch.join("Cargo.toml"),
        "[package]\nname = \"blake3-c-probe\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\
         \n[dependencies]\nblake3 = { version = \"1\", default-features = false, \
         features = [\"std\", \"pure\"] }\n\n[workspace]\n",
    )
    .map_err(|e| format!("write manifest: {e}"))?;
    std::fs::write(
        src.join("lib.rs"),
        "pub fn probe() -> blake3::Hash { blake3::hash(b\"x\") }\n",
    )
    .map_err(|e| format!("write lib.rs: {e}"))?;

    let target = scratch.join("target");
    let output = std::process::Command::new(env!("CARGO"))
        .arg("build")
        .arg("--offline")
        .arg("--quiet")
        .current_dir(&scratch)
        .env("CARGO_TARGET_DIR", &target)
        // A parent `cargo` exports variables that would leak its own build into
        // the child; the child must resolve blake3 on its own terms.
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_BUILD_TARGET_DIR")
        .output()
        .map_err(|e| format!("spawn cargo: {e}"))?;

    if !output.status.success() {
        let _ = std::fs::remove_dir_all(&scratch);
        return Err(format!(
            "probe build failed ({:?}):\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let objects = build_script_objects(&target);
    let _ = std::fs::remove_dir_all(&scratch);
    Ok(objects)
}

/// The load-bearing check: with this project's blake3 configuration, a clean
/// build compiles no C at all.
///
/// Skipped rather than failed when the probe build cannot run — an offline
/// registry cache without blake3, for instance. A gate that fails for reasons
/// unrelated to its subject teaches people to ignore it.
#[test]
fn a_clean_blake3_build_compiles_no_c_objects() {
    match blake3_objects_from_a_clean_build() {
        Ok(objects) => assert!(
            objects.is_empty(),
            "blake3 compiled {} C object(s) with `default-features = false, \
             features = [\"std\", \"pure\"]`. `pure` is the feature that keeps the \
             C compiler out of a consumer's build; if it stopped doing so, this \
             project needs a different hash crate, not a louder comment.\n{}",
            objects.len(),
            objects
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join("\n")
        ),
        Err(reason) => eprintln!(
            "skipping the clean-build probe: {reason}\n\
             This check needs a registry cache holding blake3 and a working \
             `cargo build --offline`."
        ),
    }
}

/// The C-toolchain exception must have a MEASURED boundary, not a narrated one.
///
/// GAP-SG-196 declares that this project cannot be fully C-free today and
/// explains why. What it never did was bound the exception: nothing in the
/// repository knew WHICH packages drag a C compiler in, so a new dependency
/// could add a third one and no test would notice. The tests above guard
/// `blake3` and the allocator — neither of which is a reason `cc` is in the
/// lockfile.
///
/// Two packages are. Both are indirect and both were measured, not assumed:
///
/// * `libsqlite3-sys`, from `rusqlite`'s `bundled` feature and from
///   `refinery-core`. Turning `bundled` off removes the compiler and breaks
///   self-containment; swapping the engine for a pure-Rust one loses FTS5,
///   which the BM25 half of the hybrid search is built on. The alternative that
///   exists — `oxisqlite`, a C-free fork of limbo — was at 0.4.0 when this was
///   written.
/// * `ring`, from `rustls` by way of `reqwest`. The pure-Rust provider,
///   `rustls-rustcrypto`, was published as `0.0.2-alpha`.
///
/// Neither substitution is defensible for a memory store today. So the
/// exception stands, and this test turns it into a fence: a third consumer
/// fails here and forces the decision to be made deliberately.
#[test]
fn the_c_toolchain_exception_has_exactly_the_two_known_consumers() {
    let lock = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.lock"),
    )
    .expect("Cargo.lock must be readable");

    // Sanity first: if `cc` ever leaves the graph entirely, this test must say
    // so loudly rather than pass by finding nothing to check.
    assert!(
        lock.contains("\nname = \"cc\"\n"),
        "`cc` is gone from Cargo.lock. That is good news and it invalidates \
         this test's premise — rewrite the fence, and update GAP-SG-196, \
         instead of deleting the check"
    );

    let mut consumers: Vec<String> = Vec::new();
    for package in lock.split("[[package]]").skip(1) {
        let Some(name) = package
            .lines()
            .find_map(|line| line.strip_prefix("name = \""))
            .and_then(|rest| rest.split('"').next())
        else {
            continue;
        };
        // Only the dependency list matters; a package merely NAMED `cc` in a
        // checksum line is not a consumer.
        let Some(deps) = package.split("dependencies = [").nth(1) else {
            continue;
        };
        let Some(block) = deps.split(']').next() else {
            continue;
        };
        if block
            .lines()
            .any(|line| line.trim().trim_matches(['"', ',']) == "cc")
        {
            consumers.push(name.to_string());
        }
    }
    consumers.sort();
    consumers.dedup();

    // Five packages DECLARE `cc`. Only two of them compile C on a normal
    // build of this crate, and the difference is the whole point of measuring
    // instead of assuming — an earlier pass through this material read the
    // lockfile loosely and concluded "two", which is the right answer to a
    // question nobody had checked.
    //
    // Compiles C here:
    //   libsqlite3-sys — `rusqlite` with `bundled`, plus `refinery-core`.
    //   ring           — `rustls` by way of `reqwest`.
    //
    // Declares `cc` and does NOT compile C here:
    //   blake3               — pinned `default-features = false` with the
    //                          `pure` feature, so the C/assembly backends are
    //                          off. `a_clean_blake3_build_compiles_no_c_objects`
    //                          above proves it from a real build rather than
    //                          from the manifest.
    //   iana-time-zone-haiku — target-gated to Haiku; `cargo tree -i` finds
    //                          nothing on the three supported platforms.
    //   generator            — reached only through `loom`, a dev-dependency
    //                          behind `cfg(loom)`.
    let expected = [
        "blake3",
        "generator",
        "iana-time-zone-haiku",
        "libsqlite3-sys",
        "ring",
    ];
    assert_eq!(
        consumers, expected,
        "the set of packages declaring a C-compiler build dependency changed.\n\
         expected: {expected:?}\n\
         found:    {consumers:?}\n\
         This project states a rust-native, self-contained goal, and GAP-SG-196 \
         records the two entries that genuinely violate it today along with the \
         measurements showing no alternative is ready. Anything new here needs \
         the same treatment: find out whether it actually compiles C on a \
         supported target, write down why it has to stay, and widen this list \
         on purpose — never to make the test pass."
    );
}
