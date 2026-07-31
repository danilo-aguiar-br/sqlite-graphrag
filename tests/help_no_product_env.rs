//! GAP-E2E-02 / E2E-01: help must not advertise product env or clippy Box about.
use assert_cmd::Command;

fn help_stdout(args: &[&str]) -> String {
    let mut cmd = Command::cargo_bin("sqlite-graphrag").expect("bin");
    cmd.args(args).arg("--help");
    let out = cmd.output().expect("run help");
    String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr)
}

fn assert_clean_help(label: &str, text: &str) {
    assert!(
        !text.contains("SQLITE_GRAPHRAG_"),
        "{label}: help still mentions product SQLITE_GRAPHRAG_* env:\n{text}"
    );
    assert!(
        !text.contains("Honors env") && !text.contains("Honra env"),
        "{label}: help still says Honors env:\n{text}"
    );
    assert!(
        !text.contains("Boxed to keep"),
        "{label}: help still leaks clippy Box about:\n{text}"
    );
}

#[test]
fn root_help_has_no_product_env_or_box_about() {
    assert_clean_help("root", &help_stdout(&[]));
}

#[test]
fn enrich_and_ingest_help_clean() {
    assert_clean_help("enrich", &help_stdout(&["enrich"]));
    assert_clean_help("ingest", &help_stdout(&["ingest"]));
}

#[test]
fn init_help_no_home_env_example() {
    let text = help_stdout(&["init"]);
    assert_clean_help("init", &text);
    assert!(!text.contains("HOME"), "init help still shows HOME");
}
