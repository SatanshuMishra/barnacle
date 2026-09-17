use barnacle_catalog::ShipIndex;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RecentShips(Vec<ShipIndex>);

impl RecentShips {
    pub const LIMIT: usize = 20;

    pub fn remember(&self, index: ShipIndex) -> Self {
        let kept = self.0.len().min(Self::LIMIT - 1);
        Self(
            self.0[self.0.len() - kept..]
                .iter()
                .cloned()
                .chain(std::iter::once(index))
                .collect(),
        )
    }

    pub fn contains(&self, index: &ShipIndex) -> bool {
        self.0.contains(index)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}
