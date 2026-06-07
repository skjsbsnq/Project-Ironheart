pub fn start_due_situations(state: &mut hoi4_content::SituationState, world: &hoi4_state::World) {
    state.check_start(world);
}

pub fn tick_situations(state: &mut hoi4_content::SituationState, world: &hoi4_state::World) {
    state.daily_tick(world);
}

pub fn ai_auto_intervene(state: &mut hoi4_content::SituationState, world: &mut hoi4_state::World) {
    state.ai_auto_intervene(world);
}

pub fn drain_situation_effects(
    state: &mut hoi4_content::SituationState,
) -> Vec<hoi4_content::SituationEffect> {
    state.drain_effects()
}

fn apply_runtime_country_identity(
    world: &mut hoi4_state::World,
    country: hoi4_state::CountryId,
    color: [u8; 3],
    ruling_party: &str,
) {
    let idx = country.0 as usize;
    if idx < world.countries.count {
        world.countries.colors[idx] = color;
        world.countries.ruling_party[idx] = ruling_party.to_owned();
    }
}

fn normalize_spanish_civil_war_colors(
    world: &mut hoi4_state::World,
    source_tag: &str,
    rebel_tag: &str,
    source: hoi4_state::CountryId,
) {
    if source_tag == "SPR" && rebel_tag == "SPA" {
        let src_i = source.0 as usize;
        if src_i < world.countries.count {
            world.countries.colors[src_i] = [196, 154, 66];
        }
    }
}

pub fn apply_situation_effects(app: &mut crate::App) -> bool {
    let drained = drain_situation_effects(&mut app.runtime.content.situation_state);
    let had_ownership_change = !drained.is_empty();
    for effect in drained {
        use hoi4_content::SituationEffect;
        match effect {
            SituationEffect::SplitCountry {
                source_tag,
                rebel_tag,
                rebel_color,
                rebel_party,
                fraction,
            } => {
                let Some(source) = app.world.country(&source_tag) else {
                    continue;
                };
                let rebel = app
                    .world
                    .spawn_country(&rebel_tag, rebel_color, &rebel_party);
                apply_runtime_country_identity(&mut app.world, rebel, rebel_color, &rebel_party);
                normalize_spanish_civil_war_colors(&mut app.world, &source_tag, &rebel_tag, source);
                app.runtime.econ.ensure_capacity(app.world.countries.count);
                app.runtime.research.ensure_capacity(&app.world);
                app.runtime.ai.ensure_capacity(&app.world);

                // Inherit key source-country state for the rebel.
                let src_i = source.0 as usize;
                let rbl_i = rebel.0 as usize;
                // Split manpower by fraction.
                {
                    let src_mp = app.world.manpower(source);
                    let rebel_mp = (src_mp as f64 * fraction as f64) as u64;
                    crate::subtract_manpower_from_pops(&mut app.world, source, rebel_mp);
                    crate::add_manpower_to_pops(&mut app.world, rebel, rebel_mp);
                }
                // Copy techs, equipment unlocks, subunits, buildings, and research slots.
                app.world.countries.completed_techs[rbl_i] =
                    app.world.countries.completed_techs[src_i].clone();
                app.world.countries.unlocked_equipments[rbl_i] =
                    app.world.countries.unlocked_equipments[src_i].clone();
                app.world.countries.unlocked_subunits[rbl_i] =
                    app.world.countries.unlocked_subunits[src_i].clone();
                app.world.countries.unlocked_buildings[rbl_i] =
                    app.world.countries.unlocked_buildings[src_i].clone();
                app.world.countries.research_slots[rbl_i] =
                    app.world.countries.research_slots[src_i];
                // 鐢牊膩閺夊尅绱版禒?GameData 婢跺秴鍩?source 閻ㄥ嫭膩閺夎法绮?rebel
                {
                    let data_ptr =
                        &*app.world.data as *const hoi4_data::GameData as *mut hoi4_data::GameData;
                    // SAFETY: single-threaded, no other borrows active at this point
                    unsafe {
                        if let Some(templates) =
                            (*data_ptr).division_templates.get(&source_tag).cloned()
                        {
                            (*data_ptr)
                                .division_templates
                                .insert(rebel_tag.clone(), templates);
                        }
                    }
                }

                // Find all states owned by source
                let source_states: Vec<usize> = (0..app.world.states.count)
                    .filter(|&si| app.world.states.owners[si] == source)
                    .collect();
                // Give ~fraction of states to rebel (take from the back half)
                let to_transfer = (source_states.len() as f32 * fraction).ceil() as usize;
                for &si in source_states.iter().rev().take(to_transfer) {
                    app.world.states.owners[si] = rebel;
                    app.world.states.controllers[si] = rebel;
                    for &prov in &app.world.states.provinces[si] {
                        let pi = prov.0 as usize;
                        if pi < app.world.provinces.count {
                            app.world.provinces.owners[pi] = rebel;
                            app.world.provinces.controllers[pi] = rebel;
                        }
                    }
                }
                // Set capital for rebel = first state transferred
                if let Some(&first_si) = source_states.iter().rev().take(to_transfer).last() {
                    app.world.countries.capitals[rebel.0 as usize] =
                        hoi4_state::StateId(first_si as u16);
                }
                // Create war between rebel and source
                let war_id = app.world.diplomacy.next_war_id;
                app.world.diplomacy.next_war_id += 1;
                let mut att_set = std::collections::HashSet::new();
                let mut def_set = std::collections::HashSet::new();
                att_set.insert(rebel);
                def_set.insert(source);
                app.world.diplomacy.wars.insert(
                    war_id,
                    hoi4_state::War {
                        id: war_id,
                        primary_attacker: rebel,
                        primary_defender: source,
                        attackers: att_set,
                        defenders: def_set,
                        started_at_hour: app.world.elapsed_hours,
                        attacker_war_score: 0.0,
                        defender_war_score: 0.0,
                        attacker_wargoals: vec![],
                        defender_wargoals: vec![],
                        war_join_policies: std::collections::HashMap::new(),
                    },
                );
                app.world.countries.at_war[rebel.0 as usize] = true;
                app.world.countries.at_war[source.0 as usize] = true;
                app.world.recalc_country_caches();
                println!(
                    "[situation] {} split from {} ({} states), war started",
                    rebel_tag, source_tag, to_transfer
                );
            }
            SituationEffect::SplitCountryByStates {
                source_tag,
                rebel_tag,
                rebel_color,
                rebel_party,
                rebel_states,
                manpower_fraction,
            } => {
                let Some(source) = app.world.country(&source_tag) else {
                    continue;
                };
                let rebel = app
                    .world
                    .spawn_country(&rebel_tag, rebel_color, &rebel_party);
                apply_runtime_country_identity(&mut app.world, rebel, rebel_color, &rebel_party);
                normalize_spanish_civil_war_colors(&mut app.world, &source_tag, &rebel_tag, source);
                app.runtime.econ.ensure_capacity(app.world.countries.count);
                app.runtime.research.ensure_capacity(&app.world);
                app.runtime.ai.ensure_capacity(&app.world);

                let src_i = source.0 as usize;
                let rbl_i = rebel.0 as usize;
                {
                    let src_mp = app.world.manpower(source);
                    let rebel_mp = (src_mp as f64 * manpower_fraction as f64) as u64;
                    crate::subtract_manpower_from_pops(&mut app.world, source, rebel_mp);
                    crate::add_manpower_to_pops(&mut app.world, rebel, rebel_mp);
                }
                app.world.countries.completed_techs[rbl_i] =
                    app.world.countries.completed_techs[src_i].clone();
                app.world.countries.unlocked_equipments[rbl_i] =
                    app.world.countries.unlocked_equipments[src_i].clone();
                app.world.countries.unlocked_subunits[rbl_i] =
                    app.world.countries.unlocked_subunits[src_i].clone();
                app.world.countries.unlocked_buildings[rbl_i] =
                    app.world.countries.unlocked_buildings[src_i].clone();
                app.world.countries.research_slots[rbl_i] =
                    app.world.countries.research_slots[src_i];
                {
                    let data_ptr =
                        &*app.world.data as *const hoi4_data::GameData as *mut hoi4_data::GameData;
                    // SAFETY: single-threaded, no other borrows active at this point.
                    unsafe {
                        if let Some(templates) =
                            (*data_ptr).division_templates.get(&source_tag).cloned()
                        {
                            (*data_ptr)
                                .division_templates
                                .insert(rebel_tag.clone(), templates);
                        }
                    }
                }

                let mut transferred = Vec::new();
                for game_state_id in rebel_states {
                    let Some(&state_id) = app.world.state_id_lookup.get(&game_state_id) else {
                        println!("[situation] missing state id for {rebel_tag}: {game_state_id}");
                        continue;
                    };
                    let si = state_id.0 as usize;
                    if si >= app.world.states.count || app.world.states.owners[si] != source {
                        continue;
                    }
                    app.world.states.owners[si] = rebel;
                    app.world.states.controllers[si] = rebel;
                    for &prov in &app.world.states.provinces[si] {
                        let pi = prov.0 as usize;
                        if pi < app.world.provinces.count {
                            app.world.provinces.owners[pi] = rebel;
                            app.world.provinces.controllers[pi] = rebel;
                        }
                    }
                    transferred.push(si);
                }
                retarget_building_ownership_in_states(&mut app.world, &transferred, source, rebel);
                if let Some(&first_si) = transferred.first() {
                    app.world.countries.capitals[rbl_i] = hoi4_state::StateId(first_si as u16);
                }

                let war_id = app.world.diplomacy.next_war_id;
                app.world.diplomacy.next_war_id += 1;
                let mut att_set = std::collections::HashSet::new();
                let mut def_set = std::collections::HashSet::new();
                att_set.insert(rebel);
                def_set.insert(source);
                app.world.diplomacy.wars.insert(
                    war_id,
                    hoi4_state::War {
                        id: war_id,
                        primary_attacker: rebel,
                        primary_defender: source,
                        attackers: att_set,
                        defenders: def_set,
                        started_at_hour: app.world.elapsed_hours,
                        attacker_war_score: 0.0,
                        defender_war_score: 0.0,
                        attacker_wargoals: vec![],
                        defender_wargoals: vec![],
                        war_join_policies: std::collections::HashMap::new(),
                    },
                );
                app.world.countries.at_war[rebel.0 as usize] = true;
                app.world.countries.at_war[source.0 as usize] = true;
                app.world.recalc_country_caches();
                println!(
                    "[situation] {} split from {} ({} specified states), war started",
                    rebel_tag,
                    source_tag,
                    transferred.len()
                );
            }
            SituationEffect::SplitDivisions {
                source_tag,
                rebel_tag,
                fraction,
            } => {
                // Split air, navy, and equipment; divisions are spawned separately.
                let (Some(src), Some(rbl)) = (
                    app.world.country(&source_tag),
                    app.world.country(&rebel_tag),
                ) else {
                    println!(
                                    "[situation][SplitDivisions] missing tag: source={source_tag} rebel={rebel_tag}"
                                );
                    continue;
                };

                // Split air wings by fraction.
                let mut air_count = 0u32;
                let mut total_src_air = 0u32;
                for ai in 0..app.world.air_wings.count {
                    if app.world.air_wings.owners[ai] == src {
                        total_src_air += 1;
                    }
                }
                let air_target = ((total_src_air as f32) * fraction).floor() as u32;
                for ai in 0..app.world.air_wings.count {
                    if air_count >= air_target {
                        break;
                    }
                    if app.world.air_wings.owners[ai] == src {
                        app.world.air_wings.owners[ai] = rbl;
                        air_count += 1;
                    }
                }

                let mut fleet_count = 0u32;
                let mut total_src_fleets = 0u32;
                for fi in 0..app.world.fleets.count {
                    if app.world.fleets.owners[fi] == src {
                        total_src_fleets += 1;
                    }
                }
                let fleet_target = ((total_src_fleets as f32) * fraction).floor() as u32;
                for fi in 0..app.world.fleets.count {
                    if fleet_count >= fleet_target {
                        break;
                    }
                    if app.world.fleets.owners[fi] == src {
                        app.world.fleets.owners[fi] = rbl;
                        for &ship_id in &app.world.fleets.ships[fi].clone() {
                            let si = ship_id.0 as usize;
                            if si < app.world.ships.count {
                                app.world.ships.owners[si] = rbl;
                            }
                        }
                        fleet_count += 1;
                    }
                }

                // Split equipment stockpile by fraction.
                let src_idx = src.0 as usize;
                let rbl_stockpile_idx = rbl.0 as usize;
                if src_idx < app.runtime.econ.stockpile.len()
                    && rbl_stockpile_idx < app.runtime.econ.stockpile.len()
                {
                    hoi4_logic::economy::stockpile::normalize_stockpile_keys(
                        &mut app.runtime.econ.stockpile[src_idx],
                    );
                    let snapshot: Vec<(String, f32)> = app.runtime.econ.stockpile[src_idx]
                        .iter()
                        .map(|(k, v)| (k.clone(), *v))
                        .collect();
                    for (eq_key, total) in snapshot {
                        let to_rebel = total * fraction;
                        let remaining = total - to_rebel;
                        let normalized =
                            hoi4_logic::economy::stockpile::normalize_equipment_id(&eq_key);
                        app.runtime.econ.stockpile[src_idx].insert(normalized.clone(), remaining);
                        let entry = app.runtime.econ.stockpile[rbl_stockpile_idx]
                            .entry(normalized)
                            .or_insert(0.0);
                        *entry += to_rebel;
                    }
                }

                println!(
                                "[situation] SplitDivisions: {source_tag}->{rebel_tag} air={air_count}/{total_src_air}, fleets={fleet_count}/{total_src_fleets}"
                            );
            }
            SituationEffect::SpawnFrontlineDivisions {
                tag,
                enemy_tag,
                density,
            } => {
                let (Some(tgt), Some(enm)) =
                    (app.world.country(&tag), app.world.country(&enemy_tag))
                else {
                    println!(
                                    "[situation][SpawnFrontlineDivisions] missing tag: tag={tag} enemy={enemy_tag}"
                                );
                    continue;
                };

                let tag_str = tag.clone();
                let Some(templates) = app.world.data.division_templates.get(&tag_str) else {
                    println!("[situation][SpawnFrontlineDivisions] no templates for {tag}");
                    continue;
                };
                let Some(first_tmpl) = templates.first() else {
                    println!("[situation][SpawnFrontlineDivisions] empty templates for {tag}");
                    continue;
                };
                let tmpl_idx: u16 = 0;
                let stats = hoi4_logic::military::stats::DivisionStats::aggregate(
                    first_tmpl,
                    &app.world.data,
                );

                let mut frontline_provs: Vec<hoi4_state::ProvinceId> = Vec::new();
                for si in 0..app.world.states.count {
                    if app.world.states.owners[si] != tgt {
                        continue;
                    }
                    for &p in &app.world.states.provinces[si] {
                        let pi = p.0 as usize;
                        if pi >= app.world.provinces.count {
                            continue;
                        }
                        if app.world.provinces.controllers[pi] != tgt {
                            continue;
                        }
                        if let Some(def) = app.world.map.get_province(p.0) {
                            if !matches!(def.province_type, hoi4_map::ProvinceType::Land) {
                                continue;
                            }
                        } else {
                            continue;
                        }
                        if pi >= app.world.map.adjacencies.len() {
                            continue;
                        }
                        let mut on_front = false;
                        for &adj_raw in &app.world.map.adjacencies[pi] {
                            let adj_pi = adj_raw as usize;
                            if adj_pi >= app.world.provinces.count {
                                continue;
                            }
                            let adj_ctrl = app.world.provinces.controllers[adj_pi];
                            if adj_ctrl == enm {
                                on_front = true;
                                break;
                            }
                        }
                        if on_front {
                            frontline_provs.push(p);
                        }
                    }
                }

                if frontline_provs.is_empty() {
                    for si in 0..app.world.states.count {
                        if app.world.states.owners[si] != tgt {
                            continue;
                        }
                        for &p in &app.world.states.provinces[si] {
                            let pi = p.0 as usize;
                            if pi >= app.world.provinces.count {
                                continue;
                            }
                            if let Some(def) = app.world.map.get_province(p.0) {
                                if !matches!(def.province_type, hoi4_map::ProvinceType::Land) {
                                    continue;
                                }
                            } else {
                                continue;
                            }
                            frontline_provs.push(p);
                        }
                    }
                }

                let step = density.max(1) as usize;
                let sampled: Vec<hoi4_state::ProvinceId> = frontline_provs
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| i % step == 0)
                    .map(|(_, &p)| p)
                    .collect();

                let mut spawned = 0u32;
                for (i, &prov) in sampled.iter().enumerate() {
                    let name = format!("{}. Division", i + 1);
                    app.world.divisions.push(
                        tgt,
                        prov,
                        tmpl_idx,
                        stats.max_organisation,
                        stats.max_strength,
                        name,
                    );
                    spawned += 1;
                }

                let mut transferred = 0u32;
                for di in 0..app.world.divisions.count {
                    if app.world.divisions.owners[di] != enm {
                        continue;
                    }
                    let pi = app.world.divisions.locations[di].0 as usize;
                    if pi < app.world.provinces.count && app.world.provinces.owners[pi] == tgt {
                        app.world.divisions.owners[di] = tgt;
                        app.world.divisions.destinations[di] = None;
                        transferred += 1;
                    }
                }

                println!(
                                "[situation] SpawnFrontlineDivisions: {tag} spawned {spawned}/{}, transferred {transferred} from {enemy_tag}",
                                frontline_provs.len()
                            );
            }
            SituationEffect::CreateWar { attacker, defender } => {
                let effect = hoi4_logic::scripted_effects::ScriptedDiplomaticEffect::DeclareWar {
                    attacker: attacker.clone(),
                    defender: defender.clone(),
                };
                if hoi4_logic::scripted_effects::apply_diplomatic_effect(&mut app.world, &effect)
                    .is_ok()
                {
                    println!("[situation] war created: {} vs {}", attacker, defender);
                }
            }
            SituationEffect::AnnexCountry { annexer, target } => {
                if let (Some(_ann), Some(tgt)) =
                    (app.world.country(&annexer), app.world.country(&target))
                {
                    // Clear the annexed country's military units without shifting indices.
                    let mut killed_divs = 0u32;
                    for i in 0..app.world.divisions.count {
                        if app.world.divisions.owners[i] == tgt {
                            app.world.divisions.owners[i] = hoi4_state::CountryId::NONE;
                            app.world.divisions.destinations[i] = None;
                            app.world.divisions.in_combat[i] = false;
                            app.world.divisions.strength[i] = 0.0;
                            app.world.divisions.organisation[i] = 0.0;
                            killed_divs += 1;
                        }
                    }
                    let mut killed_air = 0u32;
                    for i in 0..app.world.air_wings.count {
                        if app.world.air_wings.owners[i] == tgt {
                            app.world.air_wings.owners[i] = hoi4_state::CountryId::NONE;
                            killed_air += 1;
                        }
                    }
                    let mut killed_fleets = 0u32;
                    for i in 0..app.world.fleets.count {
                        if app.world.fleets.owners[i] == tgt {
                            app.world.fleets.owners[i] = hoi4_state::CountryId::NONE;
                            // 閸氬本顒為懜鏉垮涧 owner
                            for &ship_id in &app.world.fleets.ships[i].clone() {
                                let si = ship_id.0 as usize;
                                if si < app.world.ships.count {
                                    app.world.ships.owners[si] = hoi4_state::CountryId::NONE;
                                }
                            }
                            killed_fleets += 1;
                        }
                    }
                    let effect =
                        hoi4_logic::scripted_effects::ScriptedDiplomaticEffect::AnnexCountry {
                            annexer: annexer.clone(),
                            target: target.clone(),
                        };
                    let _ = hoi4_logic::scripted_effects::apply_diplomatic_effect(
                        &mut app.world,
                        &effect,
                    );
                    println!(
                        "[situation] {} annexed {} (units cleared: {} divs, {} air, {} fleets)",
                        annexer, target, killed_divs, killed_air, killed_fleets
                    );
                }
            }
            SituationEffect::CreateFaction { leader, name } => {
                let effect =
                    hoi4_logic::scripted_effects::ScriptedDiplomaticEffect::CreateFaction {
                        leader,
                        name,
                    };
                let _ =
                    hoi4_logic::scripted_effects::apply_diplomatic_effect(&mut app.world, &effect);
            }
            SituationEffect::AddToFaction {
                faction_leader,
                member,
            } => {
                let effect = hoi4_logic::scripted_effects::ScriptedDiplomaticEffect::AddToFaction {
                    faction_leader,
                    member,
                };
                let _ =
                    hoi4_logic::scripted_effects::apply_diplomatic_effect(&mut app.world, &effect);
            }
            SituationEffect::AddWarParticipant {
                war_leader,
                participant,
            } => {
                let effect =
                    hoi4_logic::scripted_effects::ScriptedDiplomaticEffect::AddWarParticipant {
                        war_leader,
                        participant,
                    };
                let _ =
                    hoi4_logic::scripted_effects::apply_diplomatic_effect(&mut app.world, &effect);
            }
            SituationEffect::GrantMilitaryAccess { grantor, grantee } => {
                let effect =
                    hoi4_logic::scripted_effects::ScriptedDiplomaticEffect::GrantMilitaryAccess {
                        grantor,
                        grantee,
                    };
                let _ =
                    hoi4_logic::scripted_effects::apply_diplomatic_effect(&mut app.world, &effect);
            }
            SituationEffect::SetAutonomy {
                master,
                subject,
                level,
            } => {
                let effect = hoi4_logic::scripted_effects::ScriptedDiplomaticEffect::SetAutonomy {
                    master,
                    subject,
                    level,
                };
                let _ =
                    hoi4_logic::scripted_effects::apply_diplomatic_effect(&mut app.world, &effect);
            }
            SituationEffect::TransferState { state, owner } => {
                let effect =
                    hoi4_logic::scripted_effects::ScriptedDiplomaticEffect::TransferState {
                        state,
                        owner,
                    };
                let _ =
                    hoi4_logic::scripted_effects::apply_diplomatic_effect(&mut app.world, &effect);
            }
            SituationEffect::AddCore { state, country } => {
                let effect = hoi4_logic::scripted_effects::ScriptedDiplomaticEffect::AddCore {
                    state,
                    country,
                };
                let _ =
                    hoi4_logic::scripted_effects::apply_diplomatic_effect(&mut app.world, &effect);
            }
            SituationEffect::SetWarJoinPolicy {
                war_leader,
                country,
                policy,
            } => {
                let effect =
                    hoi4_logic::scripted_effects::ScriptedDiplomaticEffect::SetWarJoinPolicy {
                        war_leader,
                        country,
                        policy,
                    };
                let _ =
                    hoi4_logic::scripted_effects::apply_diplomatic_effect(&mut app.world, &effect);
            }
            SituationEffect::AddDelayedWarParticipant {
                war_leader,
                participant,
            } => {
                let effect =
                    hoi4_logic::scripted_effects::ScriptedDiplomaticEffect::AddDelayedWarParticipant {
                        war_leader,
                        participant,
                    };
                let _ =
                    hoi4_logic::scripted_effects::apply_diplomatic_effect(&mut app.world, &effect);
            }
            SituationEffect::TriggerEvent(event_id) => {
                if !should_trigger_situation_event_for_player(
                    &event_id,
                    &app.world,
                    app.runtime.content.player,
                ) {
                    println!("[situation] skipped player-scoped event: {}", event_id);
                    continue;
                }
                // P1.2：局势触发事件时，从事件 id 前缀推断作用国家
                let effect_country = situation_effect_country_from_event_id(
                    &event_id,
                    &app.world,
                    app.runtime.content.player,
                );
                app.runtime.content.global_flags.pending_triggers.push_back(
                    hoi4_content::eval::PendingTrigger {
                        event_id: event_id.clone(),
                        effect_country,
                        display_country: effect_country,
                        source: format!("situation:{}", event_id),
                    },
                );
                println!("[situation] triggered event: {}", event_id);
            }
            SituationEffect::AddIdea { country, idea } => {
                if let Some(cid) = app.world.country(&country) {
                    let ci = cid.0 as usize;
                    if ci < app.world.countries.count
                        && !app.world.countries.ideas[ci].contains(&idea)
                    {
                        app.world.countries.ideas[ci].push(idea.clone());
                        println!("[situation] {} gained idea: {}", country, idea);
                    }
                }
            }
            SituationEffect::AddStability { country, amount } => {
                if let Some(cid) = app.world.country(&country) {
                    let ci = cid.0 as usize;
                    if ci < app.world.countries.count {
                        app.world.countries.stability[ci] =
                            (app.world.countries.stability[ci] + amount).clamp(0.0, 1.0);
                    }
                }
            }
            SituationEffect::AddWarSupport { country, amount } => {
                if let Some(cid) = app.world.country(&country) {
                    let ci = cid.0 as usize;
                    if ci < app.world.countries.count {
                        app.world.countries.war_support[ci] =
                            (app.world.countries.war_support[ci] + amount).clamp(0.0, 1.0);
                    }
                }
            }
            SituationEffect::AddOpinion {
                country_a,
                country_b,
                amount,
            } => {
                if let (Some(a), Some(b)) =
                    (app.world.country(&country_a), app.world.country(&country_b))
                {
                    app.world.diplomacy.opinions.modify(a, b, amount);
                    app.world.diplomacy.opinions.modify(b, a, amount);
                    println!("[situation] opinion {}->{}: {amount}", country_a, country_b);
                }
            }
            SituationEffect::SetCountryFlag { country, flag } => {
                if let Some(cid) = app.world.country(&country) {
                    let ci = cid.0 as usize;
                    if ci < app.world.countries.count {
                        let flag_str = format!("FLAG:{}", flag);
                        if !app.world.countries.ideas[ci].contains(&flag_str) {
                            app.world.countries.ideas[ci].push(flag_str);
                        }
                    }
                    println!("[situation] {} flag: {flag}", country);
                }
            }

            // Lend divisions by spawning volunteer units for the recipient.
            SituationEffect::LendDivisions {
                from,
                to,
                count,
                template_priority,
            } => {
                let (Some(_from_cid), Some(to_cid)) =
                    (app.world.country(&from), app.world.country(&to))
                else {
                    continue;
                };
                // Spawn in the recipient capital state's first province.
                let to_idx = to_cid.0 as usize;
                if to_idx >= app.world.countries.count {
                    continue;
                }
                let cap_state = app.world.countries.capitals[to_idx];
                if cap_state.is_none() {
                    continue;
                }
                let cap_si = cap_state.0 as usize;
                if cap_si >= app.world.states.count {
                    continue;
                }
                let provs = &app.world.states.provinces[cap_si];
                if provs.is_empty() {
                    continue;
                }
                let spawn_prov = provs[0];
                let to_tag = match app.world.country_tag(to_cid) {
                    Some(t) => t.to_string(),
                    None => continue,
                };
                let templates = app
                    .world
                    .data
                    .division_templates
                    .get(&to_tag)
                    .cloned()
                    .unwrap_or_default();
                if templates.is_empty() {
                    println!(
                                    "[situation][LendDivisions] {to} has no templates, can't spawn for {from}'s gift"
                                );
                    continue;
                }
                let chosen_idx = template_priority
                    .iter()
                    .find_map(|name| {
                        templates.iter().position(|t| {
                            t.name.contains(name) || t.regiments.iter().any(|r| r.contains(name))
                        })
                    })
                    .unwrap_or(0) as u16;
                let data_clone = app.world.data.clone();
                let mut spawned = 0u32;
                for k in 0..count {
                    let name = format!("Volunteer Brigade {} ({})", k + 1, from);
                    if hoi4_logic::military::spawn::spawn_from_template(
                        &mut app.world,
                        &data_clone,
                        to_cid,
                        spawn_prov,
                        chosen_idx,
                        name,
                    )
                    .is_some()
                    {
                        spawned += 1;
                    }
                }
                println!(
                    "[situation] LendDivisions: {from}->{to} count={spawned}/{count} prov={}",
                    spawn_prov.0
                );
            }
            SituationEffect::SpawnDivisionsInStates {
                tag,
                states,
                count_per_state,
                template_priority,
            } => {
                let Some(country) = app.world.country(&tag) else {
                    continue;
                };
                let templates = app
                    .world
                    .data
                    .division_templates
                    .get(&tag)
                    .cloned()
                    .unwrap_or_default();
                if templates.is_empty() {
                    println!("[situation][SpawnDivisionsInStates] no templates for {tag}");
                    continue;
                }
                let chosen_idx = template_priority
                    .iter()
                    .find_map(|name| {
                        templates.iter().position(|t| {
                            t.name.contains(name) || t.regiments.iter().any(|r| r.contains(name))
                        })
                    })
                    .unwrap_or(0) as u16;
                let data_clone = app.world.data.clone();
                let mut spawned = 0u32;
                for state_id in states {
                    let Some(&state) = app.world.state_id_lookup.get(&state_id) else {
                        continue;
                    };
                    let si = state.0 as usize;
                    if si >= app.world.states.count {
                        continue;
                    }
                    let Some(&province) = app.world.states.provinces[si].first() else {
                        continue;
                    };
                    for idx in 0..count_per_state {
                        let name = format!("{} Reserve {}", tag, idx + 1);
                        if hoi4_logic::military::spawn::spawn_from_template(
                            &mut app.world,
                            &data_clone,
                            country,
                            province,
                            chosen_idx,
                            name,
                        )
                        .is_some()
                        {
                            spawned += 1;
                        }
                    }
                }
                println!("[situation] SpawnDivisionsInStates: {tag} spawned {spawned}");
            }

            // Transfer available equipment from one stockpile to another.
            SituationEffect::SendEquipment {
                from,
                to,
                equipment,
                amount,
            } => {
                let (Some(from_cid), Some(to_cid)) =
                    (app.world.country(&from), app.world.country(&to))
                else {
                    continue;
                };
                let f_idx = from_cid.0 as usize;
                let t_idx = to_cid.0 as usize;
                if f_idx >= app.runtime.econ.stockpile.len()
                    || t_idx >= app.runtime.econ.stockpile.len()
                {
                    continue;
                }
                let equipment_key =
                    hoi4_logic::economy::stockpile::normalize_equipment_id(&equipment);
                let have = app.runtime.econ.stockpile[f_idx]
                    .get(&equipment_key)
                    .copied()
                    .unwrap_or(0.0);
                let send = have.min(amount).max(0.0);
                if send <= 0.0 {
                    println!(
                        "[situation] SendEquipment: {from} has no '{equipment}' to send to {to}"
                    );
                    continue;
                }
                app.runtime.econ.stockpile[f_idx].insert(equipment_key.clone(), have - send);
                *app.runtime.econ.stockpile[t_idx]
                    .entry(equipment_key)
                    .or_insert(0.0) += send;
                println!("[situation] SendEquipment: {from}->{to} {equipment} x{send:.0}");
            }

            // Apply XP support to the recipient.
            SituationEffect::SendXpBuff {
                to,
                army_xp,
                air_xp,
            } => {
                if let Some(cid) = app.world.country(&to) {
                    let ci = cid.0 as usize;
                    if ci < app.world.countries.count {
                        app.world.countries.army_xp[ci] += army_xp;
                        app.world.countries.air_xp[ci] += air_xp;
                        println!("[situation] SendXpBuff: {to} +{army_xp:.0}army +{air_xp:.0}air");
                    }
                }
            }
        }
    }
    had_ownership_change
}

fn retarget_building_ownership_in_states(
    world: &mut hoi4_state::World,
    states: &[usize],
    from: hoi4_state::CountryId,
    to: hoi4_state::CountryId,
) {
    let state_set: std::collections::HashSet<usize> = states.iter().copied().collect();
    for building in &mut world.countries.buildings_v6.buildings {
        if !state_set.contains(&(building.state.0 as usize)) {
            continue;
        }
        for share in &mut building.ownership_shares {
            match &mut share.account {
                hoi4_state::OwnershipAccount::State { country }
                | hoi4_state::OwnershipAccount::DomesticPrivate { country }
                | hoi4_state::OwnershipAccount::Cartel { country }
                | hoi4_state::OwnershipAccount::ForeignPrivate { country }
                    if *country == from =>
                {
                    *country = to;
                }
                hoi4_state::OwnershipAccount::Overlord { master, subject } => {
                    if *master == from {
                        *master = to;
                    }
                    if *subject == from {
                        *subject = to;
                    }
                }
                _ => {}
            }
        }
    }
}

/// P1.2：从事件 id 前缀推断作用国家。
/// 事件 id 格式通常为 "country_prefix.event_name"（如 "japan.pearl_harbor_strike"），
/// 或 "news.event_name"（新闻事件作用于玩家）。
/// 若无法推断，回退到玩家国家。
fn situation_effect_country_from_event_id(
    event_id: &str,
    world: &hoi4_state::World,
    player: hoi4_state::CountryId,
) -> hoi4_state::CountryId {
    if event_id == "spain.scw_choose_side" {
        return player;
    }
    if event_id.starts_with("hidden.scw_nationalist")
        || event_id == "hidden.scw_initialize_rebel_state"
    {
        if let Some(cid) = world.country("SPA") {
            return cid;
        }
    }
    if event_id.starts_with("hidden.scw_republic") {
        if let Some(cid) = world.country("SPR") {
            return cid;
        }
    }
    if event_id.starts_with("hidden.scw_cnt") {
        if let Some(cid) = world.country("CNT").or_else(|| world.country("SPR")) {
            return cid;
        }
    }
    if let Some(dot_pos) = event_id.find('.') {
        let prefix = &event_id[..dot_pos];
        if prefix != "news" {
            if let Some(cid) = world.country(prefix) {
                return cid;
            }
            // 尝试大写 tag（如 "japan" -> "JAP"）
            let upper = prefix.to_uppercase();
            if let Some(cid) = world.country(&upper) {
                return cid;
            }
            // 尝试常见 tag 映射
            let mapped = match prefix {
                "germany" => "GER",
                "japan" => "JAP",
                "china" => "CHI",
                "italy" => "ITA",
                "usa" => "USA",
                "soviet" | "sov" => "SOV",
                "france" => "FRA",
                "britain" | "england" => "ENG",
                _ => prefix,
            };
            if let Some(cid) = world.country(mapped) {
                return cid;
            }
        }
    }
    player
}

fn should_trigger_situation_event_for_player(
    event_id: &str,
    world: &hoi4_state::World,
    player: hoi4_state::CountryId,
) -> bool {
    if event_id != "spain.scw_choose_side" {
        return true;
    }
    matches!(world.country_tag(player), Some("SPR" | "SPA"))
}
