mod support;

use corpus_cartographer::embeddings::Embeddings;
use tempfile::tempdir;

#[test]
fn loads_rows_by_vocabulary_index_and_normalizes_them() {
    let directory = tempdir().unwrap();
    support::write_embeddings(
        directory.path(),
        &["salt", "pepper"],
        &[&[3.0, 4.0], &[0.0, 2.0]],
    );

    let embeddings = Embeddings::load(directory.path()).unwrap();

    assert_eq!(embeddings.len(), 2);
    assert_eq!(embeddings.dimension(), 2);
    let salt = embeddings.vector("salt").unwrap();
    assert!(
        (salt[0] - 0.6).abs() < 1e-6 && (salt[1] - 0.8).abs() < 1e-6,
        "{salt:?}"
    );
    assert_eq!(embeddings.vector("pepper").unwrap(), &[0.0, 1.0]);
    assert!(embeddings.vector("cumin").is_none());
}

#[test]
fn a_zero_row_stays_zero_instead_of_dividing_by_zero() {
    let directory = tempdir().unwrap();
    support::write_embeddings(directory.path(), &["water"], &[&[0.0, 0.0]]);

    let embeddings = Embeddings::load(directory.path()).unwrap();

    assert_eq!(embeddings.vector("water").unwrap(), &[0.0, 0.0]);
}

#[test]
fn a_missing_directory_names_the_file_it_could_not_read() {
    let directory = tempdir().unwrap();

    let error = Embeddings::load(&directory.path().join("absent"))
        .err()
        .unwrap();

    assert!(error.to_string().contains("vocab.json"), "{error}");
}

#[test]
fn a_vocabulary_that_does_not_match_the_tensor_is_refused() {
    let directory = tempdir().unwrap();
    support::write_embeddings(
        directory.path(),
        &["salt", "pepper"],
        &[&[1.0, 0.0], &[0.0, 1.0]],
    );
    std::fs::write(directory.path().join("vocab.json"), r#"{"salt": 0}"#).unwrap();

    let error = Embeddings::load(directory.path()).err().unwrap();

    assert!(error.to_string().contains("rows"), "{error}");
}

#[test]
fn neighbors_exclude_the_query_and_break_ties_by_index() {
    let embeddings = Embeddings::from_rows(
        vec![
            "anchor".into(),
            "first".into(),
            "second".into(),
            "far".into(),
        ],
        vec![1.0, 0.0, 1.0, 1.0, 1.0, 1.0, -1.0, 0.0],
        2,
    );

    let neighbors = embeddings.neighbors("anchor", 3).unwrap();

    let names: Vec<&str> = neighbors.iter().map(|(term, _)| term.as_str()).collect();
    assert_eq!(names, ["first", "second", "far"]);
    assert!(embeddings.neighbors("unknown", 3).is_none());
}
