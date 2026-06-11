use hoi4_state::{CountryId, World};

pub fn build_country_info_data(
    world: &World,
    historical_1936: &hoi4_content::Historical1936Database,
    v6_db: &hoi4_content::V6Database,
    player: CountryId,
    target: CountryId,
    has_wargoal: bool,
    instant_war: bool,
) -> Option<hoi4_ui::country_info_panel::CountryInfoData> {
    let ti = target.0 as usize;
    if ti >= world.countries.count {
        return None;
    }

    let tag = world.countries.tags.get(ti)?.clone();
    if tag.is_empty() {
        return None;
    }
    let display_name = hoi4_ui::i18n::tr(&tag).to_string();

    let (leader_name, leader_portrait_key) = head_of_state_display(world, historical_1936, target);

    let ruling = world
        .countries
        .ruling_party
        .get(ti)
        .cloned()
        .unwrap_or_default();
    let party_loc_key_long = format!("{}_{}_party_long", tag, ruling);
    let party_loc_key = format!("{}_{}_party", tag, ruling);
    let party_full_name = world
        .data
        .party_names
        .get(&party_loc_key_long)
        .or_else(|| world.data.party_names.get(&party_loc_key))
        .cloned()
        .unwrap_or_else(|| ruling.clone());
    let ruling_party_label = hoi4_ui::i18n::tr(&ruling).to_string();
    let party_popularity = party_popularity_for_country(world, target, &ruling);

    let (industrial_level, military_industrial_level) = v6_industrial_levels(world, target);
    let gdp_gbp = world
        .countries
        .treasury
        .treasuries
        .get(ti)
        .map(|t| t.gdp_gbp)
        .unwrap_or_else(|| v6_estimated_gdp_gbp(world, target));
    let construction_points = v6_construction_points(world, target);
    let population_breakdown = world.country_population_breakdown(target);
    let population = population_breakdown.governed;
    let manpower = recruitable_manpower(world, v6_db, target);
    let stability = world.countries.stability.get(ti).copied().unwrap_or(0.5);
    let war_support = world.countries.war_support.get(ti).copied().unwrap_or(0.0);

    let division_count = (0..world.divisions.count)
        .filter(|&i| world.divisions.owners[i] == target)
        .count() as u32;

    let target_tag = tag.clone();
    let our_opinion_of_target = world.diplomacy.opinions.get(player, target);
    let their_opinion_of_us = world.diplomacy.opinions.get(target, player);
    let opinion = our_opinion_of_target;
    let at_war = world.diplomacy.at_war_with(player, target);
    let player_faction = world.diplomacy.faction_of(player);
    let target_faction = world.diplomacy.faction_of(target);
    let same_faction = player_faction.is_some() && player_faction == target_faction;
    let faction_name =
        target_faction.and_then(|fid| world.diplomacy.faction(fid).map(|f| f.name.clone()));
    let autonomy = world.diplomacy.autonomy.get(&target);
    let overlord_name = autonomy.map(|a| country_display_name(world, a.master));
    let autonomy_level_name = autonomy.map(|a| autonomy_level_label(a.level).to_owned());
    let mut subject_names: Vec<String> = world
        .diplomacy
        .autonomy
        .values()
        .filter(|a| a.master == target)
        .map(|a| country_display_name(world, a.subject))
        .collect();
    subject_names.sort();

    let (ready_wargoal, justifying_wargoal, justify_progress, justify_days_remaining) =
        country_wargoal_state(world, player, target);
    let has_wargoal = has_wargoal || ready_wargoal;

    let justify_action = diplomacy_action_view(
        world,
        player,
        hoi4_logic::diplomacy::DiplomaticAction::StartJustification {
            target,
            kind: hoi4_state::WargoalType::Annex,
            target_state: None,
        },
        false,
    );
    let declare_war_action = diplomacy_action_view(
        world,
        player,
        hoi4_logic::diplomacy::DiplomaticAction::DeclareWar { target },
        instant_war,
    );
    let invite_to_faction_action = diplomacy_action_view(
        world,
        player,
        hoi4_logic::diplomacy::DiplomaticAction::InviteToFaction { target },
        false,
    );
    let request_access_action = diplomacy_action_view(
        world,
        player,
        hoi4_logic::diplomacy::DiplomaticAction::RequestMilitaryAccess { target },
        false,
    );
    let relations =
        country_relation_entries(world, player, target, has_wargoal, justifying_wargoal);
    let actions = diplomacy_action_entries(
        &target_tag,
        &justify_action,
        &declare_war_action,
        &invite_to_faction_action,
        &request_access_action,
    );

    Some(hoi4_ui::country_info_panel::CountryInfoData {
        target_tag,
        tag: tag.clone(),
        display_name,
        flag_gfx: flag_gfx_for_country(world, target),
        player_flag_gfx: flag_gfx_for_country(world, player),
        leader_name,
        leader_portrait_key,
        ruling_party: ruling,
        ruling_party_label,
        party_full_name,
        party_popularity,
        gdp_gbp,
        industrial_level,
        military_industrial_level,
        construction_points,
        division_count,
        population,
        domestic_population: population_breakdown.domestic,
        colonial_population: population_breakdown.colonial,
        governed_population: population_breakdown.governed,
        subject_population: population_breakdown.subject,
        imperial_population: population_breakdown.imperial,
        manpower,
        stability,
        war_support,
        opinion,
        our_opinion_of_target,
        their_opinion_of_us,
        at_war,
        same_faction,
        faction_name,
        overlord_name,
        subject_names,
        autonomy_level_name,
        has_wargoal,
        justifying_wargoal,
        justify_progress,
        justify_days_remaining,
        wargoals: country_wargoal_details(world, player, target),
        relation_factors: country_relation_factors(world, player, target),
        relations,
        actions,
        justify_action,
        declare_war_action,
        invite_to_faction_action,
        request_access_action,
    })
}

pub fn build_diplomacy_panel_data(
    world: &World,
    historical_1936: &hoi4_content::Historical1936Database,
    v6_db: &hoi4_content::V6Database,
    player: usize,
    selected_tag: Option<&str>,
    instant_war: bool,
) -> Option<hoi4_ui::diplomacy::DiplomacyData> {
    if player >= world.countries.count {
        return None;
    }

    let player_cid = CountryId(player as u16);
    let player_tag = world
        .countries
        .tags
        .get(player)
        .cloned()
        .unwrap_or_default();
    let player_flag_gfx = flag_gfx_for_country(world, player_cid);
    let player_faction = world
        .diplomacy
        .factions
        .iter()
        .find(|f| f.contains(player_cid))
        .map(|f| faction_entry(world, f, false));

    let countries = (0..world.countries.count)
        .filter(|&i| i != player && !world.countries.tags[i].is_empty())
        .map(|i| {
            let cid = CountryId(i as u16);
            let tag = world.countries.tags[i].clone();
            let target_tag = tag.clone();
            let display_name = hoi4_ui::i18n::tr(&tag).to_string();
            let our_opinion_of_target = world.diplomacy.opinions.get(player_cid, cid);
            let their_opinion_of_us = world.diplomacy.opinions.get(cid, player_cid);
            let opinion = our_opinion_of_target;
            let same_faction = player_faction.is_some()
                && world
                    .diplomacy
                    .factions
                    .iter()
                    .any(|f| f.contains(player_cid) && f.contains(cid));
            let (leader_name, leader_portrait_key) =
                head_of_state_display(world, historical_1936, cid);
            let (has_wargoal, justifying_wargoal, _, _) =
                country_wargoal_state(world, player_cid, cid);
            let selected = selected_tag == Some(tag.as_str());
            let detail = if selected {
                build_country_info_data(
                    world,
                    historical_1936,
                    v6_db,
                    player_cid,
                    cid,
                    has_wargoal,
                    instant_war,
                )
                .map(country_info_to_diplomacy_detail)
            } else {
                None
            };

            let ruling = world
                .countries
                .ruling_party
                .get(i)
                .cloned()
                .unwrap_or_default();
            let party_loc_key_long = format!("{}_{}_party_long", tag, ruling);
            let party_loc_key = format!("{}_{}_party", tag, ruling);
            let party_full_name = world
                .data
                .party_names
                .get(&party_loc_key_long)
                .or_else(|| world.data.party_names.get(&party_loc_key))
                .cloned()
                .unwrap_or_else(|| ruling.clone());
            let ruling_party_label = hoi4_ui::i18n::tr(&ruling).to_string();
            let party_popularity = party_popularity_for_country(world, cid, &ruling);

            hoi4_ui::diplomacy::CountryEntry {
                target_tag,
                tag: tag.clone(),
                display_name,
                flag_gfx: flag_gfx_for_country(world, cid),
                ruling_party: ruling,
                ruling_party_label,
                party_full_name,
                party_popularity,
                opinion,
                our_opinion_of_target,
                their_opinion_of_us,
                at_war: world.diplomacy.at_war_with(player_cid, cid),
                same_faction,
                relations: country_relation_entries(
                    world,
                    player_cid,
                    cid,
                    has_wargoal,
                    justifying_wargoal,
                ),
                selected,
                autonomy_summary: autonomy_summary(world, cid),
                leader_name,
                leader_portrait_key,
                detail,
            }
        })
        .collect();

    let all_factions = world
        .diplomacy
        .factions
        .iter()
        .map(|f| faction_entry(world, f, true))
        .collect();

    let mut active_wars: Vec<hoi4_ui::diplomacy::PeaceWarEntry> = world
        .diplomacy
        .wars
        .values()
        .map(|war| peace_war_entry(world, player_cid, war))
        .collect();
    active_wars.sort_by_key(|war| war.id);

    let mut requests: Vec<hoi4_ui::diplomacy::DiplomaticRequestEntry> = world
        .diplomacy
        .diplomatic_requests
        .iter()
        .filter(|request| request.from == player_cid || request.to == player_cid)
        .map(|request| hoi4_ui::diplomacy::DiplomaticRequestEntry {
            from_tag: tag_of(world, request.from),
            to_tag: tag_of(world, request.to),
            kind: diplomatic_request_kind_label(&request.kind).to_owned(),
            status: diplomatic_request_status_label(request.status).to_owned(),
        })
        .collect();
    requests.sort_by(|a, b| a.status.cmp(&b.status).then(a.kind.cmp(&b.kind)));

    Some(hoi4_ui::diplomacy::DiplomacyData {
        player_tag,
        player_flag_gfx,
        player_faction,
        all_factions,
        countries,
        active_wars,
        requests,
        world_tension: world.diplomacy.world_tension,
    })
}

pub fn head_of_state_display(
    world: &World,
    historical_1936: &hoi4_content::Historical1936Database,
    country: CountryId,
) -> (String, Option<String>) {
    if country.is_none() {
        return (String::new(), None);
    }
    let Some(tag) = world.countries.tags.get(country.0 as usize) else {
        return (String::new(), None);
    };

    if let Some(def) = historical_1936.head_of_state(tag) {
        let character = if def.character_key.is_empty() {
            None
        } else {
            world
                .data
                .characters
                .iter()
                .find(|character| character.key == def.character_key)
        };
        let name = if !def.name.is_empty() {
            def.name.clone()
        } else if let Some(character) = character {
            world
                .data
                .character_names
                .get(&character.name_loc_key)
                .cloned()
                .unwrap_or_else(|| character.name_loc_key.clone())
        } else {
            def.character_key.clone()
        };
        let leader_fallback_portrait = if !def.character_key.is_empty() || def.name.is_empty() {
            world
                .country_leader(country)
                .and_then(|leader| leader.portrait_large.clone())
        } else {
            None
        };
        let portrait = if !def.portrait_gfx.is_empty() {
            Some(def.portrait_gfx.clone())
        } else {
            character
                .and_then(|character| character.portrait_large.clone())
                .or(leader_fallback_portrait)
        };
        return (name, portrait);
    }

    match world.country_leader(country) {
        Some(def) => {
            let name = world
                .data
                .character_names
                .get(&def.name_loc_key)
                .cloned()
                .unwrap_or_else(|| def.name_loc_key.clone());
            (name, def.portrait_large.clone())
        }
        None => (String::new(), None),
    }
}

pub fn country_display_name(world: &World, country: CountryId) -> String {
    world
        .country_tag(country)
        .map(|tag| hoi4_ui::i18n::tr(tag).to_string())
        .unwrap_or_else(|| hoi4_ui::i18n::tr("unknown").to_owned())
}

pub fn autonomy_summary(world: &World, country: CountryId) -> Option<String> {
    if let Some(autonomy) = world.diplomacy.autonomy.get(&country) {
        let master = country_display_name(world, autonomy.master);
        return Some(format!(
            "{} of {}",
            autonomy_level_label(autonomy.level),
            master
        ));
    }

    let subject_count = world
        .diplomacy
        .autonomy
        .values()
        .filter(|autonomy| autonomy.master == country)
        .count();
    if subject_count > 0 {
        Some(format!("Subjects: {}", subject_count))
    } else {
        None
    }
}

fn v6_industrial_levels(world: &World, country: CountryId) -> (u32, u32) {
    if country.is_none() {
        return (0, 0);
    }
    let mut industrial = 0u32;
    let mut military = 0u32;
    for building in &world.countries.buildings_v6.buildings {
        let state_idx = building.state.0 as usize;
        if state_idx >= world.states.count
            || world.states.owners[state_idx] != country
            || building.level == 0
        {
            continue;
        }
        match building.kind {
            hoi4_state::BuildingKind::Military => military += building.level as u32,
            hoi4_state::BuildingKind::MilitaryBase => {}
            _ => industrial += building.level as u32,
        }
    }
    (industrial, military)
}

fn v6_estimated_gdp_gbp(world: &World, country: CountryId) -> f64 {
    if country.is_none() {
        return 0.0;
    }
    let ci = country.0 as usize;
    let rm_per_gbp = world
        .countries
        .treasury
        .exchange_rates
        .get(ci)
        .map(|rate| rate.rm_per_gbp.max(0.1) as f64)
        .unwrap_or(12.5);
    let mut gdp_rm = 0.0_f64;
    for building in &world.countries.buildings_v6.buildings {
        let state_idx = building.state.0 as usize;
        if state_idx >= world.states.count
            || world.states.owners[state_idx] != country
            || building.level == 0
        {
            continue;
        }
        gdp_rm += building.value_added_rm;
    }
    gdp_rm * 365.0 / rm_per_gbp
}

fn v6_construction_points(world: &World, country: CountryId) -> u32 {
    if country.is_none() {
        return 0;
    }
    hoi4_logic::economy::construction_tick::construction_cp_pool(world, country.0 as usize) as u32
}

fn recruitable_manpower(world: &World, db: &hoi4_content::V6Database, country: CountryId) -> u64 {
    let policy = hoi4_logic::military::manpower::conscription_policy(world, db, country);
    hoi4_logic::military::manpower::recruitable_manpower_breakdown(world, country, policy).total
}

fn autonomy_level_label(level: hoi4_state::AutonomyLevel) -> &'static str {
    match level {
        hoi4_state::AutonomyLevel::Integrated => "整合属地",
        hoi4_state::AutonomyLevel::IntegratedPuppet => "整合傀儡",
        hoi4_state::AutonomyLevel::Puppet => "傀儡国",
        hoi4_state::AutonomyLevel::Dominion => "自治领",
        hoi4_state::AutonomyLevel::Satellite => "卫星国",
        hoi4_state::AutonomyLevel::FreedomAssociation => "自由联合",
    }
}

fn country_wargoal_details(
    world: &World,
    player: CountryId,
    target: CountryId,
) -> Vec<hoi4_ui::diplomacy::WargoalDetailEntry> {
    world
        .diplomacy
        .pending_wargoals
        .get(&player)
        .map(|wargoals| {
            wargoals
                .iter()
                .filter(|goal| goal.target == target)
                .map(wargoal_detail_entry)
                .collect()
        })
        .unwrap_or_default()
}

fn country_wargoal_state(
    world: &World,
    player: CountryId,
    target: CountryId,
) -> (bool, bool, f32, u32) {
    world
        .diplomacy
        .pending_wargoals
        .get(&player)
        .and_then(|wgs| wgs.iter().find(|w| w.target == target))
        .map(|wg| {
            if wg.justified {
                (true, false, 1.0_f32, 0_u32)
            } else {
                let total = wg.justify_total_days.max(1.0);
                let progress = (wg.justify_progress / total).clamp(0.0, 1.0);
                let days_remaining = (wg.justify_total_days - wg.justify_progress)
                    .max(0.0)
                    .ceil() as u32;
                (false, true, progress, days_remaining)
            }
        })
        .unwrap_or((false, false, 0.0, 0))
}

fn wargoal_detail_entry(goal: &hoi4_state::Wargoal) -> hoi4_ui::diplomacy::WargoalDetailEntry {
    let total = goal.justify_total_days.max(1.0);
    let progress = if goal.justified {
        1.0
    } else {
        (goal.justify_progress / total).clamp(0.0, 1.0)
    };
    hoi4_ui::diplomacy::WargoalDetailEntry {
        kind: wargoal_kind_label(goal.kind).to_owned(),
        target_state: goal.target_state.map(|state| state.0),
        status: if goal.justified {
            "可执行".to_owned()
        } else {
            "正当化中".to_owned()
        },
        progress,
        days_remaining: if goal.justified {
            0
        } else {
            (goal.justify_total_days - goal.justify_progress)
                .max(0.0)
                .ceil() as u32
        },
        source: "外交".to_owned(),
    }
}

fn country_relation_factors(
    world: &World,
    player: CountryId,
    target: CountryId,
) -> Vec<hoi4_ui::diplomacy::RelationFactorEntry> {
    let mut factors = Vec::new();
    let opinion = world.diplomacy.opinions.get(player, target);
    factors.push(hoi4_ui::diplomacy::RelationFactorEntry {
        label: "我国评价".to_owned(),
        value: format!("{opinion:+}"),
        positive: opinion >= 0,
    });

    let reverse = world.diplomacy.opinions.get(target, player);
    factors.push(hoi4_ui::diplomacy::RelationFactorEntry {
        label: "对方评价".to_owned(),
        value: format!("{reverse:+}"),
        positive: reverse >= 0,
    });

    if world.diplomacy.at_war_with(player, target) {
        factors.push(hoi4_ui::diplomacy::RelationFactorEntry {
            label: "战争状态".to_owned(),
            value: "交战中".to_owned(),
            positive: false,
        });
    } else {
        factors.push(hoi4_ui::diplomacy::RelationFactorEntry {
            label: "战争状态".to_owned(),
            value: "和平".to_owned(),
            positive: true,
        });
    }

    match (
        world.diplomacy.faction_of(player),
        world.diplomacy.faction_of(target),
    ) {
        (Some(a), Some(b)) if a == b => factors.push(hoi4_ui::diplomacy::RelationFactorEntry {
            label: "阵营".to_owned(),
            value: "同一阵营".to_owned(),
            positive: true,
        }),
        (Some(_), Some(_)) => factors.push(hoi4_ui::diplomacy::RelationFactorEntry {
            label: "阵营".to_owned(),
            value: "不同阵营".to_owned(),
            positive: false,
        }),
        _ => factors.push(hoi4_ui::diplomacy::RelationFactorEntry {
            label: "阵营".to_owned(),
            value: "无共同阵营".to_owned(),
            positive: false,
        }),
    }

    if world.diplomacy.is_subject_of(target, player) {
        factors.push(hoi4_ui::diplomacy::RelationFactorEntry {
            label: "自治关系".to_owned(),
            value: "我国属国".to_owned(),
            positive: true,
        });
    } else if world.diplomacy.is_subject_of(player, target) {
        factors.push(hoi4_ui::diplomacy::RelationFactorEntry {
            label: "自治关系".to_owned(),
            value: "我国宗主国".to_owned(),
            positive: false,
        });
    }

    factors
}

#[derive(Default)]
struct DiplomacyRelationFacts {
    player_tag: String,
    target_tag: String,
    at_war: bool,
    same_faction: bool,
    different_faction: bool,
    target_is_subject_of_player: bool,
    target_is_overlord_of_player: bool,
    has_wargoal: bool,
    justifying_wargoal: bool,
    has_military_access: bool,
    pending_request: Option<String>,
}

fn country_relation_entries(
    world: &World,
    player: CountryId,
    target: CountryId,
    has_wargoal: bool,
    justifying_wargoal: bool,
) -> Vec<hoi4_ui::diplomacy::DiplomacyRelationEntry> {
    let player_faction = world.diplomacy.faction_of(player);
    let target_faction = world.diplomacy.faction_of(target);
    let facts = DiplomacyRelationFacts {
        player_tag: tag_of(world, player),
        target_tag: tag_of(world, target),
        at_war: world.diplomacy.at_war_with(player, target),
        same_faction: player_faction.is_some() && player_faction == target_faction,
        different_faction: matches!((player_faction, target_faction), (Some(a), Some(b)) if a != b),
        target_is_subject_of_player: world.diplomacy.is_subject_of(target, player),
        target_is_overlord_of_player: world.diplomacy.is_subject_of(player, target),
        has_wargoal,
        justifying_wargoal,
        has_military_access: world.diplomacy.has_military_access(player, target),
        pending_request: pending_request_label(world, player, target),
    };
    relation_entries_from_facts(&facts)
}

fn relation_entries_from_facts(
    facts: &DiplomacyRelationFacts,
) -> Vec<hoi4_ui::diplomacy::DiplomacyRelationEntry> {
    use hoi4_ui::diplomacy::{DiplomacyRelationEntry, DiplomacyRelationKind};

    let mut entries = Vec::new();
    if facts.at_war {
        entries.push(relation_entry(
            "at_war",
            DiplomacyRelationKind::AtWar,
            "At war",
            "GFX_relation_war_relation",
            format!("{} and {} are at war.", facts.player_tag, facts.target_tag),
            false,
        ));
    }
    if facts.same_faction {
        entries.push(relation_entry(
            "same_faction",
            DiplomacyRelationKind::SameFaction,
            "Same faction",
            "GFX_relation_faction",
            format!("{} is in the same faction.", facts.target_tag),
            true,
        ));
    } else if facts.different_faction {
        entries.push(relation_entry(
            "different_faction",
            DiplomacyRelationKind::DifferentFaction,
            "Different faction",
            "GFX_relation_faction",
            format!("{} is in another faction.", facts.target_tag),
            false,
        ));
    }
    if facts.target_is_subject_of_player {
        entries.push(relation_entry(
            "subject",
            DiplomacyRelationKind::Subject,
            "Subject",
            "GFX_relation_puppet",
            format!("{} is a subject of {}.", facts.target_tag, facts.player_tag),
            true,
        ));
    } else if facts.target_is_overlord_of_player {
        entries.push(relation_entry(
            "overlord",
            DiplomacyRelationKind::Overlord,
            "Overlord",
            "GFX_relation_master",
            format!(
                "{} is the overlord of {}.",
                facts.target_tag, facts.player_tag
            ),
            false,
        ));
    }
    if facts.has_wargoal {
        entries.push(relation_entry(
            "wargoal",
            DiplomacyRelationKind::Wargoal,
            "War goal",
            "GFX_relation_wargoal",
            format!("{} has a justified war goal.", facts.player_tag),
            false,
        ));
    } else if facts.justifying_wargoal {
        entries.push(relation_entry(
            "justifying_wargoal",
            DiplomacyRelationKind::JustifyingWargoal,
            "Justifying war goal",
            "GFX_relation_wargoal",
            format!(
                "{} is justifying against {}.",
                facts.player_tag, facts.target_tag
            ),
            false,
        ));
    }
    if facts.has_military_access {
        entries.push(relation_entry(
            "military_access",
            DiplomacyRelationKind::MilitaryAccess,
            "Military access",
            "GFX_relation_military_access",
            format!(
                "{} can move through {}.",
                facts.player_tag, facts.target_tag
            ),
            true,
        ));
    }
    if let Some(request) = facts.pending_request.as_ref() {
        entries.push(DiplomacyRelationEntry {
            id: "pending_request".to_owned(),
            kind: DiplomacyRelationKind::PendingRequest,
            label: "Pending request".to_owned(),
            sprite: "GFX_accept_decline_icon".to_owned(),
            tooltip: request.clone(),
            positive: false,
        });
    }
    entries
}

fn relation_entry(
    id: &str,
    kind: hoi4_ui::diplomacy::DiplomacyRelationKind,
    label: &str,
    sprite: &str,
    tooltip: String,
    positive: bool,
) -> hoi4_ui::diplomacy::DiplomacyRelationEntry {
    hoi4_ui::diplomacy::DiplomacyRelationEntry {
        id: id.to_owned(),
        kind,
        label: label.to_owned(),
        sprite: sprite.to_owned(),
        tooltip,
        positive,
    }
}

fn pending_request_label(world: &World, player: CountryId, target: CountryId) -> Option<String> {
    world
        .diplomacy
        .diplomatic_requests
        .iter()
        .find(|request| {
            request.status == hoi4_state::DiplomaticRequestStatus::Pending
                && request.from == player
                && request.to == target
        })
        .map(|request| format!("Pending {}", diplomatic_request_kind_label(&request.kind)))
}

fn diplomacy_action_entries(
    target_tag: &str,
    justify_action: &hoi4_ui::diplomacy::DiplomaticActionView,
    declare_war_action: &hoi4_ui::diplomacy::DiplomaticActionView,
    invite_to_faction_action: &hoi4_ui::diplomacy::DiplomaticActionView,
    request_access_action: &hoi4_ui::diplomacy::DiplomaticActionView,
) -> Vec<hoi4_ui::diplomacy::DiplomacyActionEntry> {
    use hoi4_ui::diplomacy::DiplomacyActionCommand;

    vec![
        diplomacy_action_entry(
            "declare_war",
            hoi4_ui::i18n::tr("declare_war"),
            "GFX_relation_war_relation",
            declare_war_action,
            None,
            DiplomacyActionCommand::DeclareWar {
                target_tag: target_tag.to_owned(),
            },
        ),
        diplomacy_action_entry(
            "justify_wargoal",
            hoi4_ui::i18n::tr("justify_wargoal"),
            "GFX_diplo_actions_bg",
            justify_action,
            Some(format!(
                "{:.0} PP",
                hoi4_logic::diplomacy::constants::BASE_JUSTIFY_PP_COST
            )),
            DiplomacyActionCommand::JustifyWargoal {
                target_tag: target_tag.to_owned(),
            },
        ),
        unavailable_diplomacy_action_entry("guarantee_independence", "guarantee_independence"),
        diplomacy_action_entry(
            "request_military_access",
            hoi4_ui::i18n::tr("request_access"),
            "GFX_relation_military_access",
            request_access_action,
            None,
            DiplomacyActionCommand::RequestMilitaryAccess {
                target_tag: target_tag.to_owned(),
            },
        ),
        unavailable_diplomacy_action_entry("offer_military_access", "offer_military_access"),
        unavailable_diplomacy_action_entry("request_docking_rights", "request_docking_rights"),
        unavailable_diplomacy_action_entry("offer_docking_rights", "offer_docking_rights"),
        unavailable_diplomacy_action_entry("request_airbase_access", "request_airbase_access"),
        unavailable_diplomacy_action_entry("offer_airbase_access", "offer_airbase_access"),
        unavailable_diplomacy_action_entry("improve_relations", "improve_relations"),
        unavailable_diplomacy_action_entry("send_attache", "send_attache"),
        unavailable_diplomacy_action_entry("non_aggression_pact", "non_aggression_pact"),
        diplomacy_action_entry(
            "invite_to_faction",
            hoi4_ui::i18n::tr("invite_to_faction"),
            "GFX_relation_faction",
            invite_to_faction_action,
            None,
            DiplomacyActionCommand::InviteToFaction {
                target_tag: target_tag.to_owned(),
            },
        ),
        unavailable_diplomacy_action_entry("ask_to_join_faction", "ask_to_join_faction"),
        unavailable_diplomacy_action_entry("negotiate_license", "negotiate_license"),
        unavailable_diplomacy_action_entry("lend_lease", "lend_lease"),
        unavailable_diplomacy_action_entry("request_lend_lease", "request_lend_lease"),
        unavailable_diplomacy_action_entry("trade_embargo", "trade_embargo"),
        unavailable_diplomacy_action_entry("send_volunteers", "send_volunteers"),
        unavailable_diplomacy_action_entry("expeditionary_force", "expeditionary_force"),
        unavailable_diplomacy_action_entry(
            "withdraw_expeditionary_force",
            "withdraw_expeditionary_force",
        ),
        unavailable_diplomacy_action_entry("market_access", "market_access"),
        unavailable_diplomacy_action_entry("naval_blockade", "naval_blockade"),
    ]
}

fn diplomacy_action_entry(
    id: &str,
    label: &str,
    sprite: &str,
    view: &hoi4_ui::diplomacy::DiplomaticActionView,
    cost_text: Option<String>,
    command: hoi4_ui::diplomacy::DiplomacyActionCommand,
) -> hoi4_ui::diplomacy::DiplomacyActionEntry {
    hoi4_ui::diplomacy::DiplomacyActionEntry {
        id: id.to_owned(),
        label: label.to_owned(),
        enabled: view.enabled,
        preview: view.preview.clone(),
        reason: view.reason.clone(),
        cost_text,
        sprite: sprite.to_owned(),
        command,
    }
}

fn unavailable_diplomacy_action_entry(
    id: &str,
    label_key: &str,
) -> hoi4_ui::diplomacy::DiplomacyActionEntry {
    let reason = hoi4_ui::i18n::tr("diplomacy_action_unavailable").to_owned();
    hoi4_ui::diplomacy::DiplomacyActionEntry {
        id: id.to_owned(),
        label: hoi4_ui::i18n::tr(label_key).to_owned(),
        enabled: false,
        preview: reason.clone(),
        reason: Some(reason),
        cost_text: None,
        sprite: "GFX_diplo_actions_bg".to_owned(),
        command: hoi4_ui::diplomacy::DiplomacyActionCommand::Unavailable {
            action_id: id.to_owned(),
        },
    }
}

pub fn wargoal_kind_label(kind: hoi4_state::WargoalType) -> &'static str {
    match kind {
        hoi4_state::WargoalType::Annex => "吞并",
        hoi4_state::WargoalType::TakeState => "夺取州",
        hoi4_state::WargoalType::Liberate => "解放",
        hoi4_state::WargoalType::Puppet => "扶植傀儡",
        hoi4_state::WargoalType::ToppleGovernment => "颠覆政府",
        hoi4_state::WargoalType::NavalAccess => "海军通行权",
    }
}

pub fn diplomacy_action_view(
    world: &World,
    actor: CountryId,
    action: hoi4_logic::diplomacy::DiplomaticAction,
    allow_missing_wargoal: bool,
) -> hoi4_ui::diplomacy::DiplomaticActionView {
    let preview = hoi4_logic::diplomacy::preview_action(world, actor, &action).summary;
    let availability = hoi4_logic::diplomacy::evaluate_action(world, actor, &action);
    if availability.available
        || (allow_missing_wargoal
            && matches!(
                availability.reason,
                Some(hoi4_logic::diplomacy::UnavailableReason::MissingJustifiedWargoal)
            ))
    {
        hoi4_ui::diplomacy::DiplomaticActionView::enabled(preview)
    } else {
        hoi4_ui::diplomacy::DiplomaticActionView::disabled(
            preview,
            diplomacy_unavailable_reason_text(availability.reason.as_ref()),
        )
    }
}

pub fn diplomacy_unavailable_reason_text(
    reason: Option<&hoi4_logic::diplomacy::UnavailableReason>,
) -> String {
    use hoi4_logic::diplomacy::UnavailableReason;
    match reason {
        Some(UnavailableReason::BadActor) => "行动发起方无效".to_owned(),
        Some(UnavailableReason::BadTarget) => "目标无效".to_owned(),
        Some(UnavailableReason::SelfTarget) => "不能以本国为目标".to_owned(),
        Some(UnavailableReason::AlreadyAtWar) => "已经处于战争中".to_owned(),
        Some(UnavailableReason::MissingJustifiedWargoal) => "缺少已正当化的战争目标".to_owned(),
        Some(UnavailableReason::AlreadyInFaction) => "已经加入阵营".to_owned(),
        Some(UnavailableReason::NotInFaction) => "未加入阵营".to_owned(),
        Some(UnavailableReason::TargetAlreadyInFaction) => "目标已加入阵营".to_owned(),
        Some(UnavailableReason::NoFactionToInviteFrom) => "没有可用于邀请的阵营".to_owned(),
        Some(UnavailableReason::OpinionTooLow { current, required }) => {
            format!("评价过低：当前 {current}，需要 {required}")
        }
        Some(UnavailableReason::DuplicateWargoal) => "已有相同战争目标".to_owned(),
        Some(UnavailableReason::InsufficientPoliticalPower) => "政治点数不足".to_owned(),
        Some(UnavailableReason::MissingTargetState) => "缺少目标州".to_owned(),
        Some(UnavailableReason::ExtraneousTargetState) => "不应指定目标州".to_owned(),
        Some(UnavailableReason::BadFaction) => "阵营无效".to_owned(),
        Some(UnavailableReason::WarNotFound) => "找不到战争".to_owned(),
        Some(UnavailableReason::NotWarParticipant) => "不是战争参与方".to_owned(),
        Some(UnavailableReason::DuplicatePendingRequest) => "已有待处理请求".to_owned(),
        Some(UnavailableReason::PeaceNotReady) => "和平条件尚未就绪".to_owned(),
        None => "当前不可用".to_owned(),
    }
}

fn country_info_to_diplomacy_detail(
    info: hoi4_ui::country_info_panel::CountryInfoData,
) -> hoi4_ui::diplomacy::CountryDiplomacyDetail {
    hoi4_ui::diplomacy::CountryDiplomacyDetail {
        target_tag: info.target_tag,
        tag: info.tag,
        display_name: info.display_name,
        ruling_party: info.ruling_party,
        ruling_party_label: info.ruling_party_label,
        party_full_name: info.party_full_name,
        party_popularity: info.party_popularity,
        opinion: info.opinion,
        our_opinion_of_target: info.our_opinion_of_target,
        their_opinion_of_us: info.their_opinion_of_us,
        at_war: info.at_war,
        same_faction: info.same_faction,
        faction_name: info.faction_name,
        overlord_name: info.overlord_name,
        subject_names: info.subject_names,
        autonomy_level_name: info.autonomy_level_name,
        domestic_population: info.domestic_population,
        colonial_population: info.colonial_population,
        governed_population: info.governed_population,
        subject_population: info.subject_population,
        imperial_population: info.imperial_population,
        has_wargoal: info.has_wargoal,
        justifying_wargoal: info.justifying_wargoal,
        justify_progress: info.justify_progress,
        justify_days_remaining: info.justify_days_remaining,
        wargoals: info.wargoals,
        relation_factors: info.relation_factors,
        relations: info.relations,
        actions: info.actions,
        justify_action: info.justify_action,
        declare_war_action: info.declare_war_action,
        invite_to_faction_action: info.invite_to_faction_action,
        request_access_action: info.request_access_action,
    }
}

fn faction_entry(
    world: &World,
    faction: &hoi4_state::Faction,
    localize_name: bool,
) -> hoi4_ui::diplomacy::FactionEntry {
    hoi4_ui::diplomacy::FactionEntry {
        name: if localize_name {
            super::names::localized_content_name(&faction.name, &faction.name)
        } else {
            faction.name.clone()
        },
        leader_tag: tag_of(world, faction.leader),
        member_tags: faction
            .members
            .iter()
            .map(|member| tag_of(world, *member))
            .collect(),
    }
}

fn peace_war_entry(
    world: &World,
    player: CountryId,
    war: &hoi4_state::War,
) -> hoi4_ui::diplomacy::PeaceWarEntry {
    let mut attacker_tags: Vec<String> = war.attackers.iter().map(|&c| tag_of(world, c)).collect();
    attacker_tags.sort();
    let mut defender_tags: Vec<String> = war.defenders.iter().map(|&c| tag_of(world, c)).collect();
    defender_tags.sort();

    let winning_side_goals = if war.side_of(player) == Some(hoi4_state::WarSide::Defender) {
        &war.defender_wargoals
    } else {
        &war.attacker_wargoals
    };
    let wargoals = winning_side_goals
        .iter()
        .map(|goal| hoi4_ui::diplomacy::PeaceWargoalEntry {
            claimant_tag: tag_of(world, goal.claimant),
            claimant_name: country_display_name(world, goal.claimant),
            target_tag: tag_of(world, goal.target),
            target_name: country_display_name(world, goal.target),
            kind: wargoal_kind_label(goal.kind).to_owned(),
            target_state: goal.target_state.map(|state| state.0),
        })
        .collect();

    hoi4_ui::diplomacy::PeaceWarEntry {
        id: war.id,
        primary_attacker_tag: tag_of(world, war.primary_attacker),
        primary_defender_tag: tag_of(world, war.primary_defender),
        attacker_tags,
        defender_tags,
        attacker_score: war.attacker_war_score,
        defender_score: war.defender_war_score,
        player_side: match war.side_of(player) {
            Some(hoi4_state::WarSide::Attacker) => Some(hoi4_ui::diplomacy::PeaceSide::Attacker),
            Some(hoi4_state::WarSide::Defender) => Some(hoi4_ui::diplomacy::PeaceSide::Defender),
            None => None,
        },
        wargoals,
        attacker_peace_action: diplomacy_action_view(
            world,
            player,
            hoi4_logic::diplomacy::DiplomaticAction::ResolvePeace {
                war_id: war.id,
                winning_side: hoi4_state::WarSide::Attacker,
            },
            false,
        ),
        defender_peace_action: diplomacy_action_view(
            world,
            player,
            hoi4_logic::diplomacy::DiplomaticAction::ResolvePeace {
                war_id: war.id,
                winning_side: hoi4_state::WarSide::Defender,
            },
            false,
        ),
    }
}

fn diplomatic_request_kind_label(kind: &hoi4_state::DiplomaticRequestKind) -> &'static str {
    match kind {
        hoi4_state::DiplomaticRequestKind::InviteToFaction { .. } => "邀请加入阵营",
        hoi4_state::DiplomaticRequestKind::RequestMilitaryAccess => "请求军事通行权",
        hoi4_state::DiplomaticRequestKind::OfferNonAggressionPact => "提议互不侵犯条约",
        hoi4_state::DiplomaticRequestKind::OfferPeace => "提出和平",
    }
}

fn diplomatic_request_status_label(status: hoi4_state::DiplomaticRequestStatus) -> &'static str {
    match status {
        hoi4_state::DiplomaticRequestStatus::Pending => "待处理",
        hoi4_state::DiplomaticRequestStatus::Accepted => "已接受",
        hoi4_state::DiplomaticRequestStatus::Rejected => "已拒绝",
        hoi4_state::DiplomaticRequestStatus::Expired => "已过期",
        hoi4_state::DiplomaticRequestStatus::Withdrawn => "已撤回",
    }
}

fn tag_of(world: &World, country: CountryId) -> String {
    world
        .countries
        .tags
        .get(country.0 as usize)
        .cloned()
        .unwrap_or_default()
}

fn flag_gfx_for_country(world: &World, country: CountryId) -> String {
    let index = country.0 as usize;
    let tag = world
        .countries
        .tags
        .get(index)
        .map(String::as_str)
        .unwrap_or_default();
    let ruling = world
        .countries
        .ruling_party
        .get(index)
        .map(String::as_str)
        .unwrap_or_default();
    format!("GFX_flag_{tag}_{ruling}")
}

fn party_popularity_for_country(
    world: &World,
    country: CountryId,
    ruling_party: &str,
) -> Vec<(String, f32)> {
    let index = country.0 as usize;
    let mut entries = world
        .countries
        .party_popularity
        .get(index)
        .map(|popularity| {
            popularity
                .iter()
                .map(|(ideology, value)| (ideology.clone(), *value))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    if entries.iter().all(|(_, value)| *value <= f32::EPSILON) && !ruling_party.trim().is_empty() {
        entries.push((ruling_party.to_owned(), 1.0));
    }
    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wargoal_details_are_display_ready() {
        let player = CountryId(0);
        let target = CountryId(1);
        let goal = hoi4_state::Wargoal {
            claimant: player,
            target,
            kind: hoi4_state::WargoalType::Annex,
            target_state: None,
            justified: false,
            justify_progress: 5.0,
            justify_total_days: 10.0,
        };

        let detail = wargoal_detail_entry(&goal);

        assert_eq!(detail.kind, "吞并");
        assert_eq!(detail.status, "正当化中");
        assert_eq!(detail.source, "外交");
        assert_eq!(detail.days_remaining, 5);
        assert_eq!(detail.progress, 0.5);
    }

    #[test]
    fn diplomacy_contract_relation_entries_cover_vanilla_relation_icons() {
        use hoi4_ui::diplomacy::DiplomacyRelationKind;

        let facts = DiplomacyRelationFacts {
            player_tag: "GER".to_owned(),
            target_tag: "ENG".to_owned(),
            at_war: true,
            same_faction: true,
            target_is_subject_of_player: true,
            has_wargoal: true,
            has_military_access: true,
            ..Default::default()
        };

        let entries = relation_entries_from_facts(&facts);

        assert_relation(
            &entries,
            DiplomacyRelationKind::AtWar,
            "GFX_relation_war_relation",
            false,
        );
        assert_relation(
            &entries,
            DiplomacyRelationKind::SameFaction,
            "GFX_relation_faction",
            true,
        );
        assert_relation(
            &entries,
            DiplomacyRelationKind::Subject,
            "GFX_relation_puppet",
            true,
        );
        assert_relation(
            &entries,
            DiplomacyRelationKind::Wargoal,
            "GFX_relation_wargoal",
            false,
        );
        assert_relation(
            &entries,
            DiplomacyRelationKind::MilitaryAccess,
            "GFX_relation_military_access",
            true,
        );
    }

    #[test]
    fn diplomacy_contract_action_entries_keep_disabled_reason_and_command_target() {
        use hoi4_ui::diplomacy::{DiplomacyActionCommand, DiplomaticActionView};

        let justify = DiplomaticActionView::disabled("GER starts justifying", "not enough PP");
        let declare = DiplomaticActionView::disabled("GER declares war", "missing wargoal");
        let invite = DiplomaticActionView::enabled("GER invites ENG");
        let access = DiplomaticActionView::disabled("GER requests access", "opinion too low");

        let actions = diplomacy_action_entries("ENG", &justify, &declare, &invite, &access);

        assert!(actions.len() > 4);
        let justify = actions
            .iter()
            .find(|action| action.id == "justify_wargoal")
            .expect("justify action entry");
        assert!(!justify.enabled);
        assert_eq!(justify.reason.as_deref(), Some("not enough PP"));
        assert_eq!(justify.cost_text.as_deref(), Some("50 PP"));
        assert_eq!(
            justify.command,
            DiplomacyActionCommand::JustifyWargoal {
                target_tag: "ENG".to_owned()
            }
        );

        let ids: Vec<&str> = actions.iter().map(|action| action.id.as_str()).collect();
        assert_eq!(
            ids,
            vec![
                "declare_war",
                "justify_wargoal",
                "guarantee_independence",
                "request_military_access",
                "offer_military_access",
                "request_docking_rights",
                "offer_docking_rights",
                "request_airbase_access",
                "offer_airbase_access",
                "improve_relations",
                "send_attache",
                "non_aggression_pact",
                "invite_to_faction",
                "ask_to_join_faction",
                "negotiate_license",
                "lend_lease",
                "request_lend_lease",
                "trade_embargo",
                "send_volunteers",
                "expeditionary_force",
                "withdraw_expeditionary_force",
                "market_access",
                "naval_blockade",
            ]
        );
        assert!(actions
            .iter()
            .filter(|action| matches!(action.command, DiplomacyActionCommand::Unavailable { .. }))
            .all(|action| !action.enabled));
    }

    fn assert_relation(
        entries: &[hoi4_ui::diplomacy::DiplomacyRelationEntry],
        kind: hoi4_ui::diplomacy::DiplomacyRelationKind,
        sprite: &str,
        positive: bool,
    ) {
        let entry = entries
            .iter()
            .find(|entry| entry.kind == kind)
            .unwrap_or_else(|| panic!("missing relation kind {kind:?}"));
        assert_eq!(entry.sprite, sprite);
        assert_eq!(entry.positive, positive);
    }
}
