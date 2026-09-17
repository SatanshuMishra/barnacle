#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum GameError {
    #[error(
        "no ship is eligible for tiers {min_tier} to {max_tier} with historical set to {historical}"
    )]
    EmptyPool {
        min_tier: u32,
        max_tier: u32,
        historical: bool,
    },
}
