//! 舰队属性汇总：把 fleet 中所有 ship 的战斗属性聚合为单一 FleetStats。

use hoi4_data::{GameData, ShipKind};
use hoi4_state::{FleetId, World};

#[derive(Debug, Clone, Default)]
pub struct FleetStats {
    pub total_naval_attack: f32,
    pub total_torpedo_attack: f32,
    pub total_sub_attack: f32,
    pub total_anti_air: f32,
    /// 加权平均装甲（按 hp 加权）
    pub avg_armor: f32,
    /// 最大 AP（取舰队中最强反甲）
    pub max_ap: f32,
    /// HP 总量
    pub total_hp: f32,
    /// 当前 HP（运行时；与 total_hp 比例 = 损失率）
    pub current_hp: f32,
    /// org 总量（max）
    pub total_max_org: f32,
    /// 当前 org 总量
    pub current_org: f32,
    /// 各类舰只数量
    pub screen_count: u32,
    pub capital_count: u32,
    pub carrier_count: u32,
    pub submarine_count: u32,
    pub transport_count: u32,
    /// 飞机总数（航母搭载）
    pub total_carrier_planes: u32,
    /// 视野（surface）
    pub avg_surface_visibility: f32,
}

impl FleetStats {
    pub fn ship_count(&self) -> u32 {
        self.screen_count
            + self.capital_count
            + self.carrier_count
            + self.submarine_count
            + self.transport_count
    }

    pub fn aggregate(world: &World, data: &GameData, fleet: FleetId) -> Self {
        let mut s = Self::default();
        if fleet.is_none() {
            return s;
        }
        let fi = fleet.0 as usize;
        if fi >= world.fleets.count {
            return s;
        }
        let mut armor_w = 0.0;
        let mut armor_total_w = 0.0;
        let mut surf_vis_sum = 0.0;
        let mut surf_vis_count = 0;

        for &ship_id in &world.fleets.ships[fi] {
            if ship_id.is_none() {
                continue;
            }
            let si = ship_id.0 as usize;
            if si >= world.ships.count {
                continue;
            }
            let class_key = &world.ships.class_keys[si];
            let Some(class) = data.ship_classes.get(class_key) else {
                continue;
            };

            // 战斗力按当前 HP 比例缩放（受损时输出下降）
            let hp_ratio = world.ships.hp[si] / world.ships.max_hp[si].max(1e-6);

            s.total_naval_attack += class.naval_attack * hp_ratio;
            s.total_torpedo_attack += class.torpedo_attack * hp_ratio;
            s.total_sub_attack += class.sub_attack * hp_ratio;
            s.total_anti_air += class.anti_air_attack * hp_ratio;
            s.max_ap = s.max_ap.max(class.armor_piercing);

            armor_w += class.armor * world.ships.max_hp[si];
            armor_total_w += world.ships.max_hp[si];

            s.total_hp += world.ships.max_hp[si];
            s.current_hp += world.ships.hp[si];
            s.total_max_org += world.ships.max_organisation[si];
            s.current_org += world.ships.organisation[si];
            s.total_carrier_planes += class.carrier_size;

            surf_vis_sum += class.surface_visibility;
            surf_vis_count += 1;

            match class.kind {
                ShipKind::Screen => s.screen_count += 1,
                ShipKind::Capital => s.capital_count += 1,
                ShipKind::Carrier => s.carrier_count += 1,
                ShipKind::Submarine => s.submarine_count += 1,
                ShipKind::Transport => s.transport_count += 1,
                ShipKind::Other => {}
            }
        }

        s.avg_armor = if armor_total_w > 0.0 {
            armor_w / armor_total_w
        } else {
            0.0
        };
        s.avg_surface_visibility = if surf_vis_count > 0 {
            surf_vis_sum / surf_vis_count as f32
        } else {
            0.0
        };
        s
    }
}
