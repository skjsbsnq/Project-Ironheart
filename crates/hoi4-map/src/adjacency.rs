use std::path::Path;

#[derive(Debug, Clone)]
pub struct Adjacency {
    pub from: u16,
    pub to: u16,
    pub adj_type: AdjacencyType,
    pub through: u16,
    pub rule_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdjacencyType {
    Sea,
    Land,
    River,
    Canal,
}

/// Load adjacencies.csv (straits, canals, etc.)
pub fn load_adjacencies(path: &Path) -> Result<Vec<Adjacency>, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;

    let mut adjacencies = Vec::new();

    for line in content.lines().skip(1) {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let parts: Vec<&str> = line.split(';').collect();
        if parts.len() < 5 {
            continue;
        }

        let from: u16 = match parts[0].parse() {
            Ok(v) => v,
            Err(_) => continue,
        };
        let to: u16 = match parts[1].parse() {
            Ok(v) => v,
            Err(_) => continue,
        };
        let adj_type = match parts[2] {
            "land" => AdjacencyType::Land,
            "river" => AdjacencyType::River,
            "canal" => AdjacencyType::Canal,
            _ => AdjacencyType::Sea,
        };
        let through: u16 = parts[3].parse().unwrap_or(0);
        let rule_name = if parts.len() > 8 {
            parts[8].to_owned()
        } else {
            String::new()
        };

        adjacencies.push(Adjacency {
            from,
            to,
            adj_type,
            through,
            rule_name,
        });
    }

    Ok(adjacencies)
}
