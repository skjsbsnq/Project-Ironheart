use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProvinceType {
    Land,
    Sea,
    Lake,
}

#[derive(Debug, Clone)]
pub struct ProvinceDefinition {
    pub id: u16,
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub province_type: ProvinceType,
    pub coastal: bool,
    pub terrain: String,
    pub continent: u8,
}

/// Load definition.csv → (definitions vec, rgb→id map, max_id)
pub fn load_definitions(
    path: &Path,
) -> Result<
    (
        Vec<Option<ProvinceDefinition>>,
        HashMap<(u8, u8, u8), u16>,
        u16,
    ),
    String,
> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;

    let mut rgb_to_id: HashMap<(u8, u8, u8), u16> = HashMap::new();
    let mut max_id: u16 = 0;
    let mut defs: Vec<(u16, ProvinceDefinition)> = Vec::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let parts: Vec<&str> = line.split(';').collect();
        if parts.len() < 8 {
            continue;
        }

        let id: u16 = match parts[0].parse() {
            Ok(v) => v,
            Err(_) => continue,
        };
        let r: u8 = match parts[1].parse() {
            Ok(v) => v,
            Err(_) => continue,
        };
        let g: u8 = match parts[2].parse() {
            Ok(v) => v,
            Err(_) => continue,
        };
        let b: u8 = match parts[3].parse() {
            Ok(v) => v,
            Err(_) => continue,
        };

        let province_type = match parts[4] {
            "sea" => ProvinceType::Sea,
            "lake" => ProvinceType::Lake,
            _ => ProvinceType::Land,
        };

        let coastal = parts[5] == "true";
        let terrain = parts[6].to_owned();
        let continent: u8 = parts[7].parse().unwrap_or(0);

        rgb_to_id.insert((r, g, b), id);
        if id > max_id {
            max_id = id;
        }

        defs.push((
            id,
            ProvinceDefinition {
                id,
                r,
                g,
                b,
                province_type,
                coastal,
                terrain,
                continent,
            },
        ));
    }

    let mut definitions = vec![None; max_id as usize + 1];
    for (id, def) in defs {
        definitions[id as usize] = Some(def);
    }

    Ok((definitions, rgb_to_id, max_id))
}
