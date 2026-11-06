mod support;

use corpus_cartographer::epicure::Epicure;
use tempfile::tempdir;

#[test]
fn loads_rows_by_vocabulary_index_and_normalizes_them() {
    let directory = tempdir().unwrap();
    support::write_epicure(
        directory.path(),
        &["salt", "pepper"],
        &[&[3.0, 4.0], &[0.0, 2.0]],
    );

    let epicure = Epicure::load(directory.path()).unwrap();

    assert_eq!(epicure.len(), 2);
    assert_eq!(epicure.dimension(), 2);
    let salt = epicure.vector("salt").unwrap();
    assert!(
        (salt[0] - 0.6).abs() < 1e-6 && (salt[1] - 0.8).abs() < 1e-6,
        "{salt:?}"
    );
    assert_eq!(epicure.vector("pepper").unwrap(), &[0.0, 1.0]);
    assert!(epicure.vector("cumin").is_none());
}

#[test]
fn a_zero_row_stays_zero_instead_of_dividing_by_zero() {
    let directory = tempdir().unwrap();
    support::write_epicure(directory.path(), &["water"], &[&[0.0, 0.0]]);

    let epicure = Epicure::load(directory.path()).unwrap();

    assert_eq!(epicure.vector("water").unwrap(), &[0.0, 0.0]);
}

#[test]
fn a_missing_directory_names_the_file_it_could_not_read() {
    let directory = tempdir().unwrap();

    let error = Epicure::load(&directory.path().join("absent"))
        .err()
        .unwrap();

    assert!(error.to_string().contains("vocab.json"), "{error}");
}

#[test]
fn a_vocabulary_that_does_not_match_the_tensor_is_refused() {
    let directory = tempdir().unwrap();
    support::write_epicure(
        directory.path(),
        &["salt", "pepper"],
        &[&[1.0, 0.0], &[0.0, 1.0]],
    );
    std::fs::write(directory.path().join("vocab.json"), r#"{"salt": 0}"#).unwrap();

    let error = Epicure::load(directory.path()).err().unwrap();

    assert!(error.to_string().contains("rows"), "{error}");
}

#[test]
fn neighbors_exclude_the_query_and_break_ties_by_index() {
    let epicure = Epicure::from_rows(
        vec![
            "anchor".into(),
            "first".into(),
            "second".into(),
            "far".into(),
        ],
        vec![1.0, 0.0, 1.0, 1.0, 1.0, 1.0, -1.0, 0.0],
        2,
    );

    let neighbors = epicure.neighbors("anchor", 3).unwrap();

    let names: Vec<&str> = neighbors.iter().map(|(term, _)| term.as_str()).collect();
    assert_eq!(names, ["first", "second", "far"]);
    assert!(epicure.neighbors("unknown", 3).is_none());
}
