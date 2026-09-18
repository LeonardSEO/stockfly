use stockfly_plasticity::{BioRule, EdgeMetadata, MaxRule, PlasticityRule};

#[test]
fn positive_edge_never_becomes_negative_even_under_sustained_negative_reward() {
    let mut rule = MaxRule {
        learning_rate: 10.0, // deliberately huge to try to force a sign flip
        max_multiplier: 4.0,
        activity_threshold: 0.0,
    };
    let base_magnitude = 2.0; // excitatory
    let mut current = base_magnitude;
    for _ in 0..100 {
        current = rule.update(1.0, 1.0, -1.0, base_magnitude, current);
        assert!(current >= 0.0, "excitatory edge must never go negative, got {current}");
    }
}

#[test]
fn inhibitory_edge_never_becomes_excitatory_even_under_sustained_positive_reward() {
    let mut rule = MaxRule {
        learning_rate: 10.0,
        max_multiplier: 4.0,
        activity_threshold: 0.0,
    };
    let base_magnitude = -2.0; // inhibitory
    let mut current = base_magnitude;
    for _ in 0..100 {
        current = rule.update(1.0, 1.0, 1.0, base_magnitude, current);
        assert!(current <= 0.0, "inhibitory edge must never go positive, got {current}");
    }
}

#[test]
fn positive_reward_increases_magnitude_and_negative_reward_decreases_it() {
    let mut rule = BioRule {
        learning_rate: 0.1,
        max_multiplier: 4.0,
    };
    let base_magnitude = 1.0;

    let increased = rule.update(1.0, 1.0, 1.0, base_magnitude, base_magnitude);
    assert!(increased > base_magnitude, "positive reward with active pre/post should increase magnitude");

    let decreased = rule.update(1.0, 1.0, -1.0, base_magnitude, base_magnitude);
    assert!(decreased < base_magnitude, "negative reward with active pre/post should decrease magnitude");
}

#[test]
fn magnitude_is_clamped_to_configured_multiplier() {
    let mut rule = BioRule {
        learning_rate: 100.0,
        max_multiplier: 4.0,
    };
    let base_magnitude = 1.0;
    let mut current = base_magnitude;
    for _ in 0..50 {
        current = rule.update(1.0, 1.0, 1.0, base_magnitude, current);
    }
    assert!(current <= base_magnitude.abs() * 4.0 + 1e-6);
}

#[test]
fn bio_rule_is_eligible_only_for_bio_marked_edges() {
    let rule = BioRule::default();
    assert!(rule.eligible(0, &EdgeMetadata { bio_eligible: true }));
    assert!(!rule.eligible(0, &EdgeMetadata { bio_eligible: false }));
}

#[test]
fn max_rule_is_eligible_for_every_edge() {
    let rule = MaxRule::default();
    assert!(rule.eligible(0, &EdgeMetadata { bio_eligible: false }));
    assert!(rule.eligible(0, &EdgeMetadata { bio_eligible: true }));
}

#[test]
fn max_rule_skips_updates_below_activity_threshold() {
    let mut rule = MaxRule::default();
    let base_magnitude = 1.0;
    let unchanged = rule.update(0.0, 0.0, 1.0, base_magnitude, base_magnitude);
    assert_eq!(unchanged, base_magnitude);
}
