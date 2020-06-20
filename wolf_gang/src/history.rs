/// Resource for tracking the game's current place in the history
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct CurrentHistoricalStep(pub u32);

impl Default for CurrentHistoricalStep {
    fn default() -> CurrentHistoricalStep {
        CurrentHistoricalStep(1)
    }
}

