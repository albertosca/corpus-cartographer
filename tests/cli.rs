mod support;

use std::fs;
use std::process::Command;

use tempfile::tempdir;

fn engine() -> Command {
    Command::new(env!("CARGO_BIN_EXE_corpus-cartographer"))
}

fn golden_input() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("contracts/engine-export.v2.json")
}

fn mini_embeddings_directory() -> tempfile::TempDir {
    let directory = tempdir().unwrap();
    let rows: Vec<&[f32]> = support::MINI_ROWS
        .iter()
        .map(<[f32; 4]>::as_slice)
        .collect();
    support::write_embeddings(directory.path(), &support::MINI_TERMS, &rows);
    directory
}

#[test]
fn writes_the_three_documents_and_a_summary() {
    let embeddings = mini_embeddings_directory();
    let out = tempdir().unwrap();
    let result = engine()
        .args([
            "--input",
            golden_input().to_str().unwrap(),
            "--embeddings-dir",
            embeddings.path().to_str().unwrap(),
            "--out-dir",
            out.path().to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    for name in ["links.json", "map-spec.v1.json", "map-spec.nested.json"] {
        assert!(out.path().join(name).is_file(), "{name}");
    }
    assert!(String::from_utf8_lossy(&result.stdout).contains("links="));
}

#[test]
fn an_invalid_input_fails_without_writing_links() {
    let embeddings = mini_embeddings_directory();
    let out = tempdir().unwrap();
    let input = out.path().join("bad.json");
    fs::write(&input, r#"{"schema":"other","version":1}"#).unwrap();
    let result = engine()
        .args([
            "--input",
            input.to_str().unwrap(),
            "--embeddings-dir",
            embeddings.path().to_str().unwrap(),
            "--out-dir",
            out.path().to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&result.stderr).starts_with("error: input:"));
    assert!(!out.path().join("links.json").exists());
}

#[test]
fn a_missing_epicure_names_the_file() {
    let out = tempdir().unwrap();
    let result = engine()
        .args([
            "--input",
            golden_input().to_str().unwrap(),
            "--embeddings-dir",
            "/nonexistent",
            "--out-dir",
            out.path().to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&result.stderr).contains("vocab.json"));
}

#[test]
fn a_negative_floor_is_a_value_not_another_flag() {
    let embeddings = mini_embeddings_directory();
    let out = tempdir().unwrap();
    let result = engine()
        .args([
            "--input",
            golden_input().to_str().unwrap(),
            "--embeddings-dir",
            embeddings.path().to_str().unwrap(),
            "--out-dir",
            out.path().to_str().unwrap(),
            "--floor",
            "-1",
        ])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn a_floor_outside_minus_one_to_one_is_refused() {
    let embeddings = mini_embeddings_directory();
    let out = tempdir().unwrap();
    let result = engine()
        .args([
            "--input",
            golden_input().to_str().unwrap(),
            "--embeddings-dir",
            embeddings.path().to_str().unwrap(),
            "--out-dir",
            out.path().to_str().unwrap(),
            "--floor",
            "1.5",
        ])
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&result.stderr).contains("floor"));
}

#[test]
fn runs_without_embeddings_when_no_directory_is_given() {
    let out = tempdir().unwrap();
    let result = engine()
        .args([
            "--input",
            golden_input().to_str().unwrap(),
            "--out-dir",
            out.path().to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    for name in ["links.json", "map-spec.v1.json", "map-spec.nested.json"] {
        assert!(out.path().join(name).is_file(), "{name}");
    }
}

#[test]
fn the_old_epicure_dir_flag_is_an_alias_of_embeddings_dir() {
    let embeddings = mini_embeddings_directory();
    let out = tempdir().unwrap();
    let result = engine()
        .args([
            "--input",
            golden_input().to_str().unwrap(),
            "--embeddings-dir",
            embeddings.path().to_str().unwrap(),
            "--out-dir",
            out.path().to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn a_corpus_too_big_for_dense_axes_fails_with_the_reason_on_stderr() {
    let directory = tempdir().unwrap();
    let recipes: Vec<String> = (0..5001)
        .map(|index| {
            format!(
                r#"{{"id":{id},"title":"Item","subtheme_id":11,"status":"assigned","seal":"complete","ingredients":[{{"name":"n","epicure_term":"term-{index}"}}]}}"#,
                id = index + 1
            )
        })
        .collect();
    let themes = r#"[{"id":1,"parent_id":null,"name":"A","epicure_terms":[]},{"id":11,"parent_id":1,"name":"A1","epicure_terms":[]}]"#;
    let input = directory.path().join("big.json");
    fs::write(
        &input,
        format!(
            r#"{{"schema":"recipes-cartographer/engine-export","version":2,"taxonomy_version":1,"themes":{themes},"recipes":[{}],"previous":null,"rejected_pairs":[]}}"#,
            recipes.join(",")
        ),
    )
    .unwrap();
    let result = engine()
        .args([
            "--input",
            input.to_str().unwrap(),
            "--out-dir",
            directory.path().join("out").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&result.stderr).contains("embeddings"));
    assert!(!directory.path().join("out").exists());
}
