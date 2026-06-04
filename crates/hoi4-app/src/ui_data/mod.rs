pub mod cache;
pub mod construction;
pub mod construction_commands;
pub mod country;
pub mod economy;
pub mod market;
pub mod names;
pub mod pops;

#[cfg(test)]
mod phase2_display_name_tests {
    const PLAYER_VISIBLE_DTO_SOURCES: &[(&str, &str)] = &[
        ("construction.rs", include_str!("construction.rs")),
        ("country.rs", include_str!("country.rs")),
        ("market.rs", include_str!("market.rs")),
        ("pops.rs", include_str!("pops.rs")),
    ];

    #[test]
    fn phase2_player_visible_dto_text_has_no_common_placeholders() {
        let forbidden = [
            concat!("?", "?", "?"),
            "\"Label\"",
            "\"Details\"",
            "\"Source\"",
            "\"Need\"",
            "No POP data",
            "requires attention",
            "Requires ",
            "Not buildable",
            "State limit:",
            "Market impact",
            "Blocked imports",
            "\"Justifying\"",
            "\"Diplomacy\"",
        ];

        for (file, source) in PLAYER_VISIBLE_DTO_SOURCES {
            for token in forbidden {
                assert!(
                    !source.contains(token),
                    "{file} still contains player-visible placeholder token {token:?}"
                );
            }
        }
    }

    #[test]
    fn phase2_resolver_keeps_internal_state_ids_out_of_player_dto_sources() {
        for (file, source) in PLAYER_VISIBLE_DTO_SOURCES {
            if *file == "market.rs" || *file == "pops.rs" || *file == "construction.rs" {
                assert!(
                    !source.contains(&["\"STA", "TE_"].concat())
                        && !source.contains(&["\"Sta", "te "].concat()),
                    "{file} contains a quoted internal state label"
                );
            }
        }
    }
}
