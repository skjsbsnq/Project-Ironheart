//! CR-3: OOB command hierarchy (Corps/Army/ArmyGroup) + auto-grouping.

use crate::ids::CountryId;

/// A Corps groups up to 5 divisions.
pub struct Corps {
    pub owner: CountryId,
    pub divisions: Vec<usize>, // indices into DivisionStore
}

/// An Army groups up to 3 Corps.
pub struct Army {
    pub owner: CountryId,
    pub corps: Vec<usize>, // indices into CommandHierarchy.corps
}

/// An ArmyGroup groups up to 3 Armies.
pub struct ArmyGroup {
    pub owner: CountryId,
    pub armies: Vec<usize>, // indices into CommandHierarchy.armies
}

/// OOB hierarchy storage.
#[derive(Default)]
pub struct CommandHierarchy {
    pub corps: Vec<Corps>,
    pub armies: Vec<Army>,
    pub army_groups: Vec<ArmyGroup>,
    /// For each division index, which corps it belongs to (None = unattached).
    pub div_to_corps: Vec<Option<usize>>,
}

impl CommandHierarchy {
    /// Auto-group divisions by owner. Sequential batching:
    /// 5 divs → 1 Corps, 3 Corps → 1 Army, 3 Armies → 1 ArmyGroup.
    pub fn auto_group(div_count: usize, owners: &[CountryId]) -> Self {
        let mut h = CommandHierarchy {
            div_to_corps: vec![None; div_count],
            ..Default::default()
        };
        // Group division indices by owner
        let mut by_owner: std::collections::HashMap<CountryId, Vec<usize>> = Default::default();
        for i in 0..div_count {
            if owners[i].is_none() {
                continue;
            }
            by_owner.entry(owners[i]).or_default().push(i);
        }
        for (owner, divs) in by_owner {
            let mut corps_indices: Vec<usize> = Vec::new();
            for chunk in divs.chunks(5) {
                let ci = h.corps.len();
                for &d in chunk {
                    h.div_to_corps[d] = Some(ci);
                }
                h.corps.push(Corps {
                    owner,
                    divisions: chunk.to_vec(),
                });
                corps_indices.push(ci);
            }
            let mut army_indices: Vec<usize> = Vec::new();
            for chunk in corps_indices.chunks(3) {
                let ai = h.armies.len();
                h.armies.push(Army {
                    owner,
                    corps: chunk.to_vec(),
                });
                army_indices.push(ai);
            }
            for chunk in army_indices.chunks(3) {
                h.army_groups.push(ArmyGroup {
                    owner,
                    armies: chunk.to_vec(),
                });
            }
        }
        h
    }
}
