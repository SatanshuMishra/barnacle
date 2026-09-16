use barnacle_data::extract::paper::PaperError;
use barnacle_data::extract::paper::paper_flags;

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!(
        "{}/tests/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

#[test]
fn reads_paper_flags_for_ships_only() {
    let flags = paper_flags(fixture("mini_gameparams.data")).unwrap();
    assert_eq!(flags.get("PASB008"), Some(&false));
    assert_eq!(flags.get("PASB110"), Some(&true));
    assert_eq!(flags.get("PAPT001"), None);
    assert_eq!(flags.len(), 2);
}

#[test]
fn reads_the_older_list_rooted_layout() {
    let flags = paper_flags(fixture("mini_gameparams_list_root.data")).unwrap();
    assert_eq!(flags.get("PASB110"), Some(&true));
}

#[test]
fn a_ship_without_the_flag_fails_loudly() {
    let error = paper_flags(fixture("mini_gameparams_missing_flag.data")).unwrap_err();
    assert!(
        matches!(error, PaperError::MissingFlag { ref indices } if indices == &["PJSB018".to_owned()])
    );
}

#[test]
fn bytes_that_are_not_game_params_are_a_decode_error() {
    assert!(matches!(
        paper_flags(b"not game params".to_vec()),
        Err(PaperError::Decode(_))
    ));
}

#[test]
fn a_wrapper_key_that_is_not_a_dictionary_is_an_unexpected_wrapper() {
    assert!(matches!(
        paper_flags(fixture("mini_gameparams_bad_wrapper.data")),
        Err(PaperError::UnexpectedWrapper)
    ));
}
