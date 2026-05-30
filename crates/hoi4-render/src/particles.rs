//! Phase 3.12.10 — CPU-side particle emitter system.
//!
//! Manages a pool of up to 8000 particles with 3 hardcoded emitter types
//! (smoke, fire, dust). Each emitter type defines lifetime, size, color,
//! and velocity curves. Combat / factory / scorched-earth sources feed
//! into the emitter every tick; `update()` ages particles and recycles
//! dead slots.
//!
//! ## Emitter types (hardcoded, .particle parsing deferred to Phase 7)
//!
//! | Type    | Lifetime | Size    | Color               | Velocity        |
//! |---------|----------|---------|---------------------|-----------------|
//! | Smoke   | 2.5s     | 0.3→0.8 | grey 0.6→0.2 alpha  | up 0.02 + drift |
//! | Fire    | 1.5s     | 0.2→0.5 | orange→red→black    | up 0.015        |
//! | Dust    | 3.0s     | 0.4→1.0 | tan 0.4→0.1 alpha   | slow drift      |

/// Maximum particle count (ROADMAP 10.1).
pub const MAX_PARTICLES: usize = 8000;

/// Emitter type index (0 = smoke, 1 = fire, 2 = dust).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum EmitterType {
    Smoke = 0,
    Fire = 1,
    Dust = 2,
}

impl EmitterType {
    pub const COUNT: u8 = 3;

    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::Smoke,
            1 => Self::Fire,
            2 => Self::Dust,
            _ => Self::Smoke,
        }
    }
}

/// Per-emitter-type configuration (hardcoded; .particle parsing deferred).
#[derive(Debug, Clone, Copy)]
pub struct EmitterConfig {
    pub lifetime_min: f32,
    pub lifetime_max: f32,
    pub size_start: f32,
    pub size_end: f32,
    pub color_start: [f32; 4],
    pub color_end: [f32; 4],
    pub velocity_base: [f32; 3],
    pub velocity_drift: f32,
}

impl EmitterConfig {
    pub fn for_type(t: EmitterType) -> Self {
        match t {
            EmitterType::Smoke => Self {
                lifetime_min: 2.0,
                lifetime_max: 3.0,
                size_start: 0.3,
                size_end: 0.8,
                color_start: [0.55, 0.55, 0.55, 0.6],
                color_end: [0.35, 0.35, 0.35, 0.0],
                velocity_base: [0.0, 0.02, 0.0],
                velocity_drift: 0.005,
            },
            EmitterType::Fire => Self {
                lifetime_min: 1.0,
                lifetime_max: 1.8,
                size_start: 0.2,
                size_end: 0.5,
                color_start: [1.0, 0.6, 0.1, 0.8],
                color_end: [0.5, 0.1, 0.0, 0.0],
                velocity_base: [0.0, 0.015, 0.0],
                velocity_drift: 0.003,
            },
            EmitterType::Dust => Self {
                lifetime_min: 2.5,
                lifetime_max: 4.0,
                size_start: 0.4,
                size_end: 1.0,
                color_start: [0.65, 0.55, 0.40, 0.4],
                color_end: [0.50, 0.40, 0.30, 0.0],
                velocity_base: [0.0, 0.005, 0.0],
                velocity_drift: 0.008,
            },
        }
    }
}

/// A single live particle.
#[derive(Debug, Clone, Copy)]
pub struct Particle {
    pub world_pos: [f32; 3],
    pub age: f32,
    pub lifetime: f32,
    pub size: f32,
    pub color: [f32; 4],
    pub velocity: [f32; 3],
    pub emitter_type: EmitterType,
    pub rotation: f32,
    pub alive: bool,
}

/// GPU-side instance data (48 bytes, std140-friendly).
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ParticleInstance {
    pub world_pos: [f32; 3],
    pub size: f32,
    pub color: [f32; 4],
    pub age_lifetime: [f32; 2],
    pub rotation: f32,
    pub _pad: f32,
}

impl Default for ParticleInstance {
    fn default() -> Self {
        Self {
            world_pos: [0.0; 3],
            size: 0.0,
            color: [0.0; 4],
            age_lifetime: [0.0, 1.0],
            rotation: 0.0,
            _pad: 0.0,
        }
    }
}

/// CPU-side particle emitter pool.
pub struct ParticleEmitter {
    particles: Vec<Particle>,
    count: u32,
}

impl ParticleEmitter {
    pub fn new() -> Self {
        let mut particles = Vec::with_capacity(MAX_PARTICLES);
        for _ in 0..MAX_PARTICLES {
            particles.push(Particle {
                world_pos: [0.0; 3],
                age: 0.0,
                lifetime: 1.0,
                size: 0.0,
                color: [0.0; 4],
                velocity: [0.0; 3],
                emitter_type: EmitterType::Smoke,
                rotation: 0.0,
                alive: false,
            });
        }
        Self {
            particles,
            count: 0,
        }
    }

    /// Emit a single particle at `world_pos` with the given emitter type.
    /// Returns false if the pool is full and the particle was discarded.
    pub fn emit(&mut self, world_pos: [f32; 3], emitter_type: EmitterType) -> bool {
        let slot = self.find_dead_slot();
        let slot = match slot {
            Some(i) => i,
            None => return false,
        };

        let was_alive = self.particles[slot].alive;

        let cfg = EmitterConfig::for_type(emitter_type);
        let lifetime = cfg.lifetime_min
            + (cfg.lifetime_max - cfg.lifetime_min) * pseudo_random(self.count as f32 * 0.123);
        let drift_x = (pseudo_random(self.count as f32 * 0.456) - 0.5) * cfg.velocity_drift;
        let drift_z = (pseudo_random(self.count as f32 * 0.789) - 0.5) * cfg.velocity_drift;
        let rotation = pseudo_random(self.count as f32 * 1.234) * std::f32::consts::TAU;

        self.particles[slot] = Particle {
            world_pos,
            age: 0.0,
            lifetime,
            size: cfg.size_start,
            color: cfg.color_start,
            velocity: [
                cfg.velocity_base[0] + drift_x,
                cfg.velocity_base[1],
                cfg.velocity_base[2] + drift_z,
            ],
            emitter_type,
            rotation,
            alive: true,
        };
        if !was_alive {
            self.count += 1;
        }
        true
    }

    /// Update all particles by `dt` seconds. Returns the new alive count.
    pub fn update(&mut self, dt: f32) -> u32 {
        let mut alive_count = 0u32;
        for p in &mut self.particles {
            if !p.alive {
                continue;
            }
            p.age += dt;
            if p.age >= p.lifetime {
                p.alive = false;
                self.count = self.count.saturating_sub(1);
                continue;
            }
            let t = p.age / p.lifetime;
            let cfg = EmitterConfig::for_type(p.emitter_type);
            p.world_pos[0] += p.velocity[0] * dt;
            p.world_pos[1] += p.velocity[1] * dt;
            p.world_pos[2] += p.velocity[2] * dt;
            p.size = cfg.size_start + (cfg.size_end - cfg.size_start) * t;
            for i in 0..4 {
                p.color[i] = cfg.color_start[i] + (cfg.color_end[i] - cfg.color_start[i]) * t;
            }
            p.rotation += dt * 0.3;
            alive_count += 1;
        }
        alive_count
    }

    /// Collect alive particles into GPU instance data.
    pub fn collect_instances(&self, out: &mut Vec<ParticleInstance>) {
        out.clear();
        for p in &self.particles {
            if !p.alive {
                continue;
            }
            out.push(ParticleInstance {
                world_pos: p.world_pos,
                size: p.size,
                color: p.color,
                age_lifetime: [p.age, p.lifetime],
                rotation: p.rotation,
                _pad: 0.0,
            });
        }
    }

    /// Current alive particle count.
    pub fn alive_count(&self) -> u32 {
        self.count
    }

    fn find_dead_slot(&self) -> Option<usize> {
        if self.count as usize >= MAX_PARTICLES {
            let oldest = self
                .particles
                .iter()
                .enumerate()
                .filter(|(_, p)| p.alive)
                .max_by(|a, b| {
                    let ra = a.1.age / a.1.lifetime;
                    let rb = b.1.age / b.1.lifetime;
                    ra.partial_cmp(&rb).unwrap_or(std::cmp::Ordering::Equal)
                });
            match oldest {
                Some((idx, _)) => Some(idx),
                None => None,
            }
        } else {
            self.particles.iter().position(|p| !p.alive)
        }
    }
}

fn pseudo_random(seed: f32) -> f32 {
    let s = seed.sin() * 43758.5453;
    s - s.floor()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emit_and_update() {
        let mut em = ParticleEmitter::new();
        assert!(em.emit([1.0, 0.0, 2.0], EmitterType::Smoke));
        assert_eq!(em.alive_count(), 1);
        em.update(0.5);
        assert_eq!(em.alive_count(), 1);
        em.update(10.0);
        assert_eq!(em.alive_count(), 0);
    }

    #[test]
    fn max_particles_respected() {
        let mut em = ParticleEmitter::new();
        for i in 0..MAX_PARTICLES + 100 {
            em.emit([0.0, 0.0, 0.0], EmitterType::Smoke);
        }
        assert!(em.alive_count() <= MAX_PARTICLES as u32);
    }

    #[test]
    fn collect_instances_matches_alive() {
        let mut em = ParticleEmitter::new();
        em.emit([1.0, 0.0, 0.0], EmitterType::Fire);
        em.emit([2.0, 0.0, 0.0], EmitterType::Dust);
        let mut instances = Vec::new();
        em.collect_instances(&mut instances);
        assert_eq!(instances.len(), 2);
    }

    #[test]
    fn instance_size_is_48_bytes() {
        assert_eq!(std::mem::size_of::<ParticleInstance>(), 48);
    }

    #[test]
    fn emitter_config_smoke_sane() {
        let cfg = EmitterConfig::for_type(EmitterType::Smoke);
        assert!(cfg.lifetime_max > cfg.lifetime_min);
        assert!(cfg.color_start[3] > cfg.color_end[3]);
    }
}
