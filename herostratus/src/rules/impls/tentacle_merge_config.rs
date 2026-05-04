/// Shared configuration for the octopus and cthulhu merge rules.
///
/// `octopus_threshold` is the minimum number of parents for a merge to grant the octopus
/// achievement. `cthulhu_threshold` is the minimum number of parents for the cthulhu
/// achievement (and also the upper bound for the octopus rule). A merge with parents in
/// `[octopus_threshold, cthulhu_threshold)` grants octopus only; a merge with parents
/// >= cthulhu_threshold grants cthulhu only.
#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct TentacleMergeConfig {
    pub octopus_threshold: usize,
    pub cthulhu_threshold: usize,
}

impl Default for TentacleMergeConfig {
    fn default() -> Self {
        Self {
            octopus_threshold: 3,
            cthulhu_threshold: 8,
        }
    }
}
