/// 3-letter country tag (e.g. "GER", "ENG", "SOV")
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CountryTag(pub String);

impl CountryTag {
    pub fn new(tag: &str) -> Self {
        Self(tag.to_uppercase())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Country color (RGB)
#[derive(Debug, Clone, Copy)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// Loaded country data
#[derive(Debug, Clone)]
pub struct Country {
    pub tag: CountryTag,
    pub color: Color,
    pub graphical_culture: String,
    pub capital: u16,
    pub ruling_party: String,
    pub technologies: Vec<String>,
}
