#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgba8 {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba8 {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }
}

pub mod palette {
    use super::Rgba8;

    pub const CANVAS: Rgba8 = Rgba8::rgb(0x05, 0x06, 0x06);
    pub const PANEL: Rgba8 = Rgba8::rgb(0x12, 0x14, 0x12);
    pub const PANEL_DEEP: Rgba8 = Rgba8::rgb(0x08, 0x09, 0x08);
    pub const HAIRLINE: Rgba8 = Rgba8::rgb(0x35, 0x37, 0x32);
    pub const BRASS: Rgba8 = Rgba8::rgb(0x9b, 0x7a, 0x3f);
    pub const TEXT: Rgba8 = Rgba8::rgb(0xd7, 0xcc, 0xa7);
    pub const MUTED: Rgba8 = Rgba8::rgb(0x88, 0x87, 0x7d);
    pub const GOOD: Rgba8 = Rgba8::rgb(0x6c, 0xc0, 0x70);
    pub const WARN: Rgba8 = Rgba8::rgb(0xf0, 0xb8, 0x50);
    pub const BAD: Rgba8 = Rgba8::rgb(0xd8, 0x58, 0x4c);
}

pub mod spacing {
    pub const S0: f32 = 0.0;
    pub const S1: f32 = 2.0;
    pub const S2: f32 = 4.0;
    pub const S3: f32 = 6.0;
    pub const S4: f32 = 8.0;
    pub const S5: f32 = 12.0;
    pub const S6: f32 = 16.0;
    pub const S7: f32 = 24.0;
    pub const S8: f32 = 32.0;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextRole {
    Title,
    Heading,
    Body,
    Caption,
    Numeric,
}

impl TextRole {
    pub const fn size(self) -> f32 {
        match self {
            Self::Title => 28.0,
            Self::Heading => 18.0,
            Self::Body => 13.0,
            Self::Caption => 11.0,
            Self::Numeric => 13.0,
        }
    }
}
