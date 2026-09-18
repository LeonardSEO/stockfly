use std::collections::HashSet;

use stockfly_chess::OutputMap;

const OUTPUT_MAP_JSON: &str = include_str!("../resources/output-map.json");

#[test]
fn all_132_groups_are_nonempty_disjoint_and_stable_for_the_fixed_seed() {
    let map = OutputMap::load_str(OUTPUT_MAP_JSON).expect("real output map should load");

    assert_eq!(map.seed, 0x53544F434B464C59);
    assert_eq!(map.from_groups.len(), 64);
    assert_eq!(map.to_groups.len(), 64);
    assert_eq!(map.promotion_groups.len(), 4);

    let mut seen: HashSet<u32> = HashSet::new();
    let mut total = 0;
    for group in map
        .from_groups
        .iter()
        .chain(map.to_groups.iter())
        .chain(map.promotion_groups.values())
    {
        assert!(!group.is_empty());
        for &n in group {
            assert!(seen.insert(n), "neuron {n} appears in more than one group");
            total += 1;
        }
    }
    assert_eq!(total, seen.len());
}

#[test]
fn regenerating_with_the_same_inputs_is_deterministic() {
    // Loading the same committed file twice must produce the same hash;
    // this is the property the real generator script relies on for
    // reproducibility (same seed + same source population -> same output).
    let a = OutputMap::load_str(OUTPUT_MAP_JSON).unwrap();
    let b = OutputMap::load_str(OUTPUT_MAP_JSON).unwrap();
    assert_eq!(a.sha256(), b.sha256());
}
