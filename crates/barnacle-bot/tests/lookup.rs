mod common;

use barnacle_bot::lookup::Directory;
use barnacle_bot::lookup::MAX_SUGGESTIONS;
use barnacle_catalog::Ship;
use barnacle_catalog::ShipName;
use common::catalog;
use common::fleet;
use common::index;
use common::ship;

fn directory() -> Directory {
    let konig = Ship {
        name: Some(ShipName {
            short: "König".to_owned(),
            full: Some("König Albert".to_owned()),
        }),
        ..ship("PGSB105", "König", 5, "konig")
    };
    let nameless = Ship {
        name: None,
        ..ship("PXSX999", "unused", 5, "nameless")
    };
    Directory::new(&catalog(vec![
        ship("PBSC507", "Belfast", 7, "belfast"),
        ship("PBSC528", "Belfast '43", 8, "belfast-43"),
        ship("PJSB018", "Yamato", 10, "yamato"),
        ship("PASB510", "Montana B", 10, "montana"),
        konig,
        nameless,
    ]))
}

fn indexes(typed: &str) -> Vec<String> {
    directory()
        .suggest(typed)
        .into_iter()
        .map(|suggestion| suggestion.index.to_string())
        .collect()
}

#[test]
fn names_starting_with_the_text_come_before_names_containing_it() {
    let directory = Directory::new(&catalog(vec![
        ship("PASB001", "Westfast", 5, "a"),
        ship("PBSC507", "Belfast", 7, "b"),
        ship("PASB002", "Fastback", 5, "c"),
    ]));
    let labels: Vec<String> = directory
        .suggest("fast")
        .into_iter()
        .map(|suggestion| suggestion.label)
        .collect();
    assert_eq!(
        labels,
        [
            "Fastback (V, Japan)",
            "Belfast (VII, Japan)",
            "Westfast (V, Japan)"
        ]
    );
}

#[test]
fn matching_ignores_accents_case_and_punctuation() {
    assert_eq!(indexes("konig"), ["PGSB105"]);
    assert_eq!(indexes("KÖNIG AL"), ["PGSB105"]);
    assert_eq!(indexes("belfast 43"), ["PBSC528"]);
}

#[test]
fn excluded_ships_are_suggested_and_nameless_ships_are_not() {
    assert_eq!(indexes("montana"), ["PASB510"]);
    assert_eq!(indexes("").len(), 5);
    assert!(!indexes("").contains(&"PXSX999".to_owned()));
}

#[test]
fn at_most_twenty_five_suggestions_are_returned() {
    let directory = Directory::new(&catalog(fleet(30)));
    assert_eq!(directory.suggest("hull").len(), MAX_SUGGESTIONS);
    assert_eq!(MAX_SUGGESTIONS, 25);
}

#[test]
fn typed_text_resolves_as_an_id_then_as_an_exact_name() {
    let directory = directory();
    assert_eq!(directory.resolve("PJSB018"), Some(&index("PJSB018")));
    assert_eq!(directory.resolve(" pjsb018 "), Some(&index("PJSB018")));
    assert_eq!(directory.resolve("Belfast '43"), Some(&index("PBSC528")));
    assert_eq!(directory.resolve("konig albert"), Some(&index("PGSB105")));
    assert_eq!(directory.resolve("belf"), None);
    assert_eq!(directory.resolve("PXSX999"), None);
    assert_eq!(directory.resolve("..."), None);
}
