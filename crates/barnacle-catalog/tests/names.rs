use barnacle_catalog::names::clean_answer;
use barnacle_catalog::names::name_tokens;

#[test]
fn clean_answer_removes_spacing_and_punctuation() {
    assert_eq!(clean_answer("Des Moines"), "desmoines");
    assert_eq!(clean_answer("W. Virginia '41"), "wvirginia41");
    assert_eq!(clean_answer("Z-52"), "z52");
    assert_eq!(clean_answer("Sun Yat-Sen"), "sunyatsen");
    assert_eq!(clean_answer("  Kremlin  "), "kremlin");
}

#[test]
fn clean_answer_transliterates_accents_and_non_breaking_spaces() {
    assert_eq!(clean_answer("Ägir"), "agir");
    assert_eq!(clean_answer("Kongō"), "kongo");
    assert_eq!(clean_answer("Prins van\u{a0}Oranje"), "prinsvanoranje");
}

#[test]
fn name_tokens_splits_into_lowercase_ascii_words() {
    assert_eq!(name_tokens("AL Ägir"), ["al", "agir"]);
    assert_eq!(
        name_tokens("Prins van\u{a0}Oranje Golden"),
        ["prins", "van", "oranje", "golden"]
    );
    assert_eq!(name_tokens("Belfast '43"), ["belfast", "43"]);
    assert_eq!(name_tokens("AL Sov. Rossiya"), ["al", "sov", "rossiya"]);
    assert_eq!(name_tokens("Z-52"), ["z52"]);
}
