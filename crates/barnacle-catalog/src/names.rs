use deunicode::deunicode;

const REMOVED: [char; 4] = ['-', '.', '\'', ','];

fn ascii_without_punctuation(text: &str) -> String {
    let spaced: String = text
        .chars()
        .map(|c| if c.is_whitespace() { ' ' } else { c })
        .collect();
    deunicode(&spaced)
        .chars()
        .filter(|c| !REMOVED.contains(c))
        .collect()
}

pub fn clean_answer(text: &str) -> String {
    ascii_without_punctuation(text)
        .chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect()
}

pub fn name_tokens(text: &str) -> Vec<String> {
    ascii_without_punctuation(text)
        .split_whitespace()
        .map(str::to_lowercase)
        .collect()
}
