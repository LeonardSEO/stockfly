use std::time::Duration;

#[derive(Debug, Clone, Copy)]
pub struct Preset {
    pub name: &'static str,
    pub wall_clock_budget: Duration,
    pub settle_steps: u32,
}

pub fn lookup(name: &str) -> Option<Preset> {
    match name {
        "smoke" => Some(Preset { name: "smoke", wall_clock_budget: Duration::from_secs(10 * 60), settle_steps: 8 }),
        "quick" => Some(Preset { name: "quick", wall_clock_budget: Duration::from_secs(30 * 60), settle_steps: 12 }),
        "standard" => Some(Preset { name: "standard", wall_clock_budget: Duration::from_secs(2 * 60 * 60), settle_steps: 16 }),
        "overnight" => Some(Preset { name: "overnight", wall_clock_budget: Duration::from_secs(6 * 60 * 60), settle_steps: 24 }),
        _ => None,
    }
}
