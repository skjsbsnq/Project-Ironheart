use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

pub fn signature(world: &hoi4_state::World) -> u64 {
    let mut h = DefaultHasher::new();
    world.countries.trade.routes.len().hash(&mut h);
    for route in &world.countries.trade.routes {
        route.id.hash(&mut h);
        route.importer.hash(&mut h);
        route.exporter.hash(&mut h);
        route.good_id.hash(&mut h);
        route.kind.hash(&mut h);
        route.port_state.hash(&mut h);
        route.throughput.to_bits().hash(&mut h);
        route.is_blockaded.hash(&mut h);
    }
    world.countries.capitals.hash(&mut h);
    h.finish()
}
