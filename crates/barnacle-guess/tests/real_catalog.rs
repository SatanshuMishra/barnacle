use std::path::Path;

use barnacle_catalog::Catalog;
use barnacle_catalog::Tier;
use barnacle_catalog::curation::CurationConfig;
use barnacle_catalog::names::clean_answer;
use barnacle_guess::RecentShips;
use barnacle_guess::RoundOptions;
use barnacle_guess::ShipBook;
use rand::SeedableRng;
use rand::rngs::StdRng;

#[test]
fn every_ship_in_the_real_pool_can_be_drawn_and_answered() {
    let Some(dir) = std::env::var_os("BARNACLE_TEST_CATALOG_DIR") else {
        eprintln!(
            "skipping: set BARNACLE_TEST_CATALOG_DIR to the absolute path of a built catalog"
        );
        return;
    };
    let catalog =
        Catalog::from_json(&std::fs::read_to_string(Path::new(&dir).join("catalog.json")).unwrap())
            .unwrap();
    let curation = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../curation/ships.toml");
    let config = CurationConfig::from_toml(&std::fs::read_to_string(curation).unwrap()).unwrap();
    let book = ShipBook::new(&catalog, &config);

    let every_tier = RoundOptions::new(Tier::new(1).ok(), Tier::new(11).ok(), None);
    let pool = book.pool(&every_tier);
    assert!(!pool.is_empty());
    for index in &pool {
        let answers = book.answers(index, &every_tier);
        let name = catalog
            .get(index)
            .and_then(|ship| ship.name.as_ref())
            .unwrap();
        assert!(
            answers.contains(&clean_answer(&name.short)),
            "{index} does not accept its own short name {:?}",
            name.short
        );
        assert!(
            answers.iter().all(|answer| answer.is_ascii()
                && !answer.is_empty()
                && !answer
                    .chars()
                    .any(|c| c.is_whitespace() || c.is_ascii_uppercase())),
            "{index} has an answer that is not cleaned: {answers:?}"
        );
    }

    let default_pool = book.pool(&RoundOptions::default()).len();
    let historical_pool = book.pool(&RoundOptions::new(None, None, Some(true))).len();
    assert!(historical_pool < default_pool);
    let draw = book
        .draw(
            &RoundOptions::default(),
            &RecentShips::default(),
            &mut StdRng::seed_from_u64(1),
        )
        .unwrap();
    assert!(!draw.answers().is_empty());
    eprintln!(
        "pool sizes: every tier {}, default {default_pool}, default historical {historical_pool}",
        pool.len()
    );
}
