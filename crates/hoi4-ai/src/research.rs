use hoi4_content::{TechCategoryDef, TechUnlockDef, V6Database};
use hoi4_logic::research::ResearchState;
use hoi4_state::{CountryId, World};

use crate::constants::*;
use crate::profile::AiProfile;
use crate::scoring::{rank, Scored, Scorer};

#[derive(Debug, Clone)]
pub struct ResearchDecision {
    pub tech_key: String,
    pub slot_idx: usize,
}

#[derive(Debug, Clone)]
pub struct ResearchScore {
    pub tech_key: String,
    pub score: f32,
    pub reason: String,
}

pub fn evaluate_research(
    world: &World,
    country: CountryId,
    research_state: &ResearchState,
    personality: &AiProfile,
    db: &V6Database,
) -> Vec<Scored<ResearchDecision>> {
    let ci = country.0 as usize;
    if ci >= world.countries.count || ci >= research_state.count {
        return Vec::new();
    }

    let idle_slots: Vec<usize> = research_state.slots[ci]
        .iter()
        .enumerate()
        .filter(|(_, s)| s.is_idle())
        .map(|(i, _)| i)
        .collect();
    if idle_slots.is_empty() {
        return Vec::new();
    }

    let completed = &world.countries.completed_techs[ci];
    let current_year = world.date.year;
    let scorer = Scorer::new(world.random_seed.wrapping_add(ci as u64 + 999));
    let mut results = Vec::new();

    for tech in &db.technologies {
        let tech_key = &tech.id;
        if completed.iter().any(|t| t == tech_key) {
            continue;
        }
        if research_state.slots[ci]
            .iter()
            .any(|s| s.current_tech() == Some(tech_key))
        {
            continue;
        }
        if !tech.prereqs.is_empty()
            && !tech
                .prereqs
                .iter()
                .all(|p| completed.iter().any(|t| t == p))
        {
            continue;
        }

        let mut score = if tech.research_cost > 0.0 {
            100.0 / tech.research_cost
        } else {
            50.0
        };
        let mut reasons = vec![format!("cost={:.1}", tech.research_cost)];

        let mut economy_unlocks = 0;
        let mut building_unlocks = 0;
        let mut law_unlocks = 0;
        for unlock in &tech.unlocks {
            match unlock {
                TechUnlockDef::Good(_) | TechUnlockDef::PM(_) => economy_unlocks += 1,
                TechUnlockDef::Building(_) => building_unlocks += 1,
                TechUnlockDef::Law(_, _) => law_unlocks += 1,
            }
        }
        if economy_unlocks > 0 {
            score += TECH_EQUIPMENT_BONUS + economy_unlocks as f32 * 2.0;
            reasons.push(format!("econ_unlocks={economy_unlocks}"));
        }
        if building_unlocks > 0 {
            score += TECH_BUILDING_BONUS;
            reasons.push(format!("building={building_unlocks}"));
        }
        if law_unlocks > 0 {
            score += TECH_SUBUNIT_BONUS;
            reasons.push(format!("law={law_unlocks}"));
        }

        if matches!(
            tech.category,
            TechCategoryDef::MilitaryDoctrine | TechCategoryDef::Naval
        ) {
            score += personality.doctrine_priority * DOCTRINE_BONUS;
            reasons.push("doctrine".to_owned());
        }

        match tech.category {
            TechCategoryDef::Industry
            | TechCategoryDef::Chemistry
            | TechCategoryDef::Electrical
            | TechCategoryDef::Metallurgy => {
                score += TECH_INDUSTRY_BONUS;
                reasons.push("industry".to_owned());
            }
            TechCategoryDef::Aviation => {
                score += TECH_EQUIPMENT_BONUS * 0.5;
                reasons.push("aviation".to_owned());
            }
            TechCategoryDef::MilitaryDoctrine => {
                score += TECH_INFANTRY_BONUS * 0.5;
            }
            _ => {}
        }

        if tech.start_year > 0 && current_year < tech.start_year {
            let years_ahead = (tech.start_year - current_year) as f32;
            if years_ahead > AHEAD_OF_TIME_MAX_YEARS {
                let penalty = 0.25_f32.powf(years_ahead - AHEAD_OF_TIME_MAX_YEARS);
                score *= penalty;
                reasons.push(format!("AOT-{years_ahead:.0}y"));
            } else {
                score *= 1.0 - years_ahead * 0.3;
                reasons.push(format!("early-{years_ahead:.0}y"));
            }
        }

        score = scorer.jitter(score, tech_key);

        for &slot_idx in &idle_slots {
            results.push(Scored::new(
                ResearchDecision {
                    tech_key: tech_key.clone(),
                    slot_idx,
                },
                score,
                reasons.join(", "),
            ));
        }
    }

    rank(results)
}

pub fn apply_research_decision(
    research_state: &mut ResearchState,
    world: &World,
    country: CountryId,
    decision: &ResearchDecision,
    db: &V6Database,
) {
    let _ = research_state.start(world, country, &decision.tech_key, db);
}
