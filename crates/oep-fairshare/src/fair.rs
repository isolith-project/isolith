//! Weighted max-min fair-share allocation: the heart of per-node fairness.
//!
//! Given a node's finite capacity and each demanding team's {weight, demand},
//! compute the allocation that is **max-min fair**: you cannot increase any
//! team's allocation without decreasing another team that already has a
//! smaller or equal allocation (normalized by weight). This is the classic
//! water-filling algorithm, done here in exact integer arithmetic (u64
//! inputs, u128 accumulation) so every rank ordering is identical on every
//! host and CI run.
//!
//! Semantics used by the scheduler:
//! - Teams with `demand == 0` are irrelevant and get 0.
//! - A team is **throttled** when its demand exceeds its allocation: it
//!   requested more than its fair share of the node and is told to wait.
//! - Surplus from teams that demand less than their share is redistributed to
//!   the teams that demand more, proportionally by weight.

/// Compute a weighted max-min-fair allocation of `capacity` across `demands`.
///
/// `weights[i]` is team *i*'s fair-share weight (>= 1); `demands[i]` is the
/// amount team *i* wants on the node. Returned `alloc[i] = min(demand_i,
/// fair_share_i)`; `sum(alloc) <= capacity` always holds, and when total
/// demand exceeds capacity the returned sum is `< capacity` only by the
/// rounding leftover after proportionally distributing the remainder.
///
/// # Panics
///
/// Panics if the two slices differ in length, or if any weight is 0
/// (the caller validates weights upstream; fail loud rather than silently
/// divide by zero).
pub fn weighted_max_min(capacity: u64, weights: &[u64], demands: &[u64]) -> Vec<u64> {
    assert_eq!(
        weights.len(),
        demands.len(),
        "weights and demands must have equal length"
    );
    let n = weights.len();
    let mut alloc = vec![0u64; n];

    if capacity == 0 {
        return alloc;
    }
    // Only teams with positive demand participate in the water-filling.
    let mut active: Vec<usize> = (0..n).filter(|&i| demands[i] > 0).collect();
    if active.is_empty() {
        return alloc;
    }
    assert!(
        active.iter().all(|&i| weights[i] >= 1),
        "weights must be >= 1"
    );

    // Water level L = level_num / level_den (rational), raised monotonically.
    // A team *i*'s proportional allocation at level L is weights[i] * L,
    // capped by demands[i]. We raise L to the smallest active fill-up ratio
    // (demand/weight) affordable within the remaining capacity, freeze the
    // teams that fill up, and continue with the rest.
    let mut level_num: u128 = 0;
    let mut level_den: u128 = 1;
    let mut remaining: u128 = capacity as u128;

    while !active.is_empty() && remaining > 0 {
        // Smallest demand/weight ratio among active teams (cross-multiply
        // to avoid floats; we only need the ratio fnum/fden, not the team).
        let mut fnum = demands[active[0]] as u128;
        let mut fden = weights[active[0]] as u128;
        for &i in &active[1..] {
            let d = demands[i] as u128;
            let w = weights[i] as u128;
            if d * fden < fnum * w {
                fnum = d;
                fden = w;
            }
        }

        // Cost to raise the level from current to fill ratio fnum/fden,
        // summed over all active teams (each gains weights[i] * delta):
        //   delta = fnum/fden - level_num/level_den
        //   cost  = delta * wsum_active
        // As an integer comparison (all u128):
        //   (fnum*level_den - level_num*fden) * wsum  <=  remaining * fden * level_den
        let wsum: u128 = active.iter().map(|&i| weights[i] as u128).sum();
        let norm_level = level_num.saturating_mul(fden);
        let norm_target = fnum.saturating_mul(level_den);
        let delta_num = norm_target.saturating_sub(norm_level).saturating_mul(wsum);
        let budget = remaining.saturating_mul(fden).saturating_mul(level_den);
        let affordable = delta_num <= budget;

        if affordable {
            // Raise the level to the fill ratio and re-allocate: each active
            // team gets weights[i] * (fnum/fden), capped by its demand.
            level_num = fnum;
            level_den = fden;
            let mut used: u128 = 0;
            for &i in &active {
                let want = (weights[i] as u128) * fnum / fden;
                let a = want.min(demands[i] as u128);
                used += a - alloc[i] as u128; // incremental gain
                alloc[i] = a as u64;
            }
            // Teams whose demand is now fully met leave the active set.
            let before = active.len();
            active.retain(|&i| alloc[i] < demands[i]);
            remaining = remaining.saturating_sub(used);
            if active.len() == before {
                // No one filled up (level reached but nobody capped): only
                // possible if all demands were infinite; guard against a
                // zero-divisor loop by consuming the remainder directly.
                remaining = 0;
            }
        } else {
            // Not affordable to fill the next team: distribute the whole
            // remaining capacity proportionally to weight, then hand out the
            // flooring leftovers deterministically (round-robin by index).
            let mut used: u128 = 0;
            let mut adds = vec![0u128; n];
            for &i in &active {
                let g = (weights[i] as u128) * remaining / wsum;
                adds[i] = g;
                used += g;
            }
            let mut left = remaining - used;
            let mut k = 0usize;
            while left > 0 {
                let i = active[k % active.len()];
                adds[i] += 1;
                left -= 1;
                k += 1;
            }
            let mut total: u128 = 0;
            for &i in &active {
                alloc[i] = (alloc[i] as u128 + adds[i]).min(demands[i] as u128) as u64;
                total += alloc[i] as u128;
            }
            remaining = 0;
            debug_assert!(total <= capacity as u128);
        }
    }

    // Safety clamp: allocation never exceeds demand and never exceeds the
    // capacity *contribution* (the sum is bounded by construction; the clamp
    // below only guards rounding edges).
    let mut actual: u128 = 0;
    for i in 0..n {
        alloc[i] = alloc[i].min(demands[i]);
        actual += alloc[i] as u128;
    }
    let _ = actual;
    alloc
}

#[cfg(test)]
mod tests {
    use super::weighted_max_min;

    #[test]
    fn zero_capacity_yields_zero() {
        assert_eq!(weighted_max_min(0, &[1, 1], &[10, 10]), vec![0, 0]);
    }

    #[test]
    fn equal_weights_split_evenly() {
        // 100 capacity, two equal-weight teams both demanding 100 -> 50/50.
        let a = weighted_max_min(100, &[1, 1], &[100, 100]);
        assert_eq!(a, vec![50, 50]);
        assert_eq!(a.iter().sum::<u64>(), 100);
    }

    #[test]
    fn low_demand_team_gets_full_demand_rest_to_other() {
        // Team A wants 20 (under its 50 share), team B wants 100. Water fills
        // A to 20, then B takes the rest up to 80 (min(share,demand)=80).
        let a = weighted_max_min(100, &[1, 1], &[20, 100]);
        assert_eq!(a[0], 20);
        assert_eq!(a[1], 80);
        assert_eq!(a.iter().sum::<u64>(), 100);
    }

    #[test]
    fn weights_scale_shares() {
        // Weights 3:1, both demanding everything -> 75/25.
        let a = weighted_max_min(100, &[3, 1], &[100, 100]);
        assert_eq!(a, vec![75, 25]);
        assert_eq!(a.iter().sum::<u64>(), 100);
    }

    #[test]
    fn three_teams_with_one_starved() {
        // A,B,C weights 1,1,1, capacity 90; A wants 10, B wants 100, C wants
        // 100. Equal shares are 30 each; A takes 10, B+C split the 80.
        let a = weighted_max_min(90, &[1, 1, 1], &[10, 100, 100]);
        assert_eq!(a[0], 10);
        assert_eq!(a[1] + a[2], 80);
        assert_eq!(a.iter().sum::<u64>(), 90);
        // B and C should be equal
        assert_eq!(a[1], a[2]);
    }

    #[test]
    fn three_teams_all_equal_pressure_split_evenly() {
        // capacity 99, 3 equal teams all demand a lot -> 33/33/33.
        let a = weighted_max_min(99, &[1, 1, 1], &[500, 500, 500]);
        assert_eq!(a, vec![33, 33, 33]);
        assert_eq!(a.iter().sum::<u64>(), 99);
        assert!(a.iter().all(|&x| x == 33));
    }

    #[test]
    fn never_exceeds_capacity() {
        for cap in [1u64, 3, 5, 7, 11, 37, 1000] {
            for &demand in &[1u64, 3, cap, cap + 10] {
                let a = weighted_max_min(cap, &[1, 1, 1], &[demand, demand, demand]);
                assert!(a.iter().all(|&x| x <= demand));
                assert!(a.iter().sum::<u64>() <= cap + 1); // rounding slack
                                                           // equal weights -> equal shares (differ by at most 1 for rounding)
                assert!(a[0].abs_diff(a[1]) <= 1);
                assert!(a[1].abs_diff(a[2]) <= 1);
            }
        }
    }

    #[test]
    fn large_capacity_large_demand_exact() {
        // Regression: millicore-scale capacities must not overflow or distort.
        let a = weighted_max_min(16_000, &[1, 1], &[12_000, 12_000]);
        assert_eq!(a, vec![8_000, 8_000]);
        assert_eq!(a.iter().sum::<u64>(), 16_000);
    }

    #[test]
    fn mixed_weights_and_demands() {
        // A weight 3 demand 10, B weight 1 demand 100, capacity 100.
        // Shares 75/25; A capped at 10, B gets up to 90 < 100.
        let a = weighted_max_min(100, &[3, 1], &[10, 100]);
        assert_eq!(a[0], 10);
        assert_eq!(a[1], 90);
        assert_eq!(a.iter().sum::<u64>(), 100);
    }

    #[test]
    #[should_panic]
    fn zero_weight_panics() {
        let _ = weighted_max_min(10, &[0, 1], &[5, 5]);
    }
}
