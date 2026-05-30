//! 强类型 ID — 防止把 ProvinceId 当成 StateId 用。
//! 全部 Copy 类型，零开销。

use std::fmt;

macro_rules! id_type {
    ($name:ident, $inner:ty) => {
        #[repr(transparent)]
        #[derive(
            Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
        )]
        pub struct $name(pub $inner);

        impl $name {
            pub const NONE: Self = Self(<$inner>::MAX);
            pub fn raw(self) -> $inner {
                self.0
            }
            pub fn is_none(self) -> bool {
                self.0 == <$inner>::MAX
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                if self.is_none() {
                    write!(f, "{}(NONE)", stringify!($name))
                } else {
                    write!(f, "{}({})", stringify!($name), self.0)
                }
            }
        }
    };
}

id_type!(ProvinceId, u16);
id_type!(StateId, u16);
id_type!(CountryId, u16);
id_type!(DivisionId, u32);
id_type!(FactionId, u32);
id_type!(ShipId, u32);
id_type!(FleetId, u32);
id_type!(SeaRegionId, u32);
id_type!(AirWingId, u32);
