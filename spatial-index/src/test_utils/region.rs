//! Randomised checks for the [`RegionMembershipPredicate`] contract.
//!
//! The contract is conservative rejection: `Reject` asserts that *no* point of the box is a
//! member. Nothing downstream of a traversal can detect a violation — the results simply come
//! back short — so it is worth checking rather than assuming, and worth checking for every
//! implementation rather than only the first one.
//!
//! ```no_run
//! # use spatial_index::region::{RegionMembershipPredicate, RegionMembershipStatus};
//! # use spatial_index::test_utils::region::assert_conservative;
//! # struct Ball;
//! # impl RegionMembershipPredicate<f32, 3> for Ball {
//! #     fn classify(&self, _: &[f32; 3], _: &[f32; 3]) -> RegionMembershipStatus {
//! #         RegionMembershipStatus::Undecided
//! #     }
//! #     fn contains(&self, _: &[f32; 3]) -> bool { false }
//! # }
//! assert_conservative(&Ball, [-10.0, 10.0], 0.05..=3.0, 20_000, 0x5eed);
//! ```

use std::ops::RangeInclusive;

use crate::region::{RegionMembershipPredicate, RegionMembershipStatus};

/// How finely each box is sampled per axis when looking for a member inside it.
///
/// The check can only fail in one direction — it finds a member the predicate rejected — so a
/// coarse grid makes it weaker, never wrong. Six is enough to catch a cone slicing a cell while
/// missing its corners, which is the case a pointwise predicate gets wrong.
const SAMPLES_PER_AXIS: usize = 6;

/// Assert that `predicate` never rejects a box containing one of its own members.
///
/// Boxes are drawn uniformly: centres within `extent`, half-widths within `half_width`. Returns
/// how many of the `trials` boxes actually held a member, which the caller should sanity-check —
/// a run where nothing was ever visible proves nothing and usually means the geometry and the
/// sampling volume do not overlap.
///
/// # Panics
///
/// If a box containing a member is classified [`RegionMembershipStatus::Reject`].
pub fn assert_conservative<P>(
    predicate: &P,
    extent: [f32; 2],
    half_width: RangeInclusive<f32>,
    trials: usize,
    seed: u64,
) -> usize
where
    P: RegionMembershipPredicate<f32, 3>,
{
    let mut rng = Lcg::new(seed);
    let mut with_members = 0usize;

    for _ in 0..trials {
        let centre = [
            rng.range(extent[0], extent[1]),
            rng.range(extent[0], extent[1]),
            rng.range(extent[0], extent[1]),
        ];
        let half = rng.range(*half_width.start(), *half_width.end());
        let lo = [centre[0] - half, centre[1] - half, centre[2] - half];
        let hi = [centre[0] + half, centre[1] + half, centre[2] + half];

        let Some(witness) = find_member(predicate, &lo, &hi) else {
            continue;
        };
        with_members += 1;

        assert_ne!(
            predicate.classify(&lo, &hi),
            RegionMembershipStatus::Reject,
            "rejected a box containing a member: lo={lo:?} hi={hi:?} member={witness:?}"
        );
    }

    with_members
}

/// Assert that `contains` agrees with an independent reference predicate at every sampled point.
///
/// Worth having separately: `classify` can be conservative and still correct while `contains`
/// disagrees with the caller's own notion of membership, in which case the query answers a
/// subtly different question from the one being validated elsewhere.
///
/// # Panics
///
/// On the first disagreement.
pub fn assert_contains_matches<P, R>(
    predicate: &P,
    reference: R,
    extent: [f32; 2],
    trials: usize,
    seed: u64,
) where
    P: RegionMembershipPredicate<f32, 3>,
    R: Fn(&[f32; 3]) -> bool,
{
    let mut rng = Lcg::new(seed);
    for _ in 0..trials {
        let point = [
            rng.range(extent[0], extent[1]),
            rng.range(extent[0], extent[1]),
            rng.range(extent[0], extent[1]),
        ];
        assert_eq!(
            predicate.contains(&point),
            reference(&point),
            "contains disagreed with the reference at {point:?}"
        );
    }
}

fn find_member<P>(predicate: &P, lo: &[f32; 3], hi: &[f32; 3]) -> Option<[f32; 3]>
where
    P: RegionMembershipPredicate<f32, 3>,
{
    let at = |n: usize, low: f32, high: f32| {
        low + (high - low) * (n as f32 / (SAMPLES_PER_AXIS - 1) as f32)
    };
    for i in 0..SAMPLES_PER_AXIS {
        for j in 0..SAMPLES_PER_AXIS {
            for k in 0..SAMPLES_PER_AXIS {
                let point = [
                    at(i, lo[0], hi[0]),
                    at(j, lo[1], hi[1]),
                    at(k, lo[2], hi[2]),
                ];
                if predicate.contains(&point) {
                    return Some(point);
                }
            }
        }
    }
    None
}

/// A deterministic generator, so a failure names a box that can be replayed.
struct Lcg(u64);

impl Lcg {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn range(&mut self, low: f32, high: f32) -> f32 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let unit = (self.0 >> 40) as f32 / (1u64 << 24) as f32;
        low + (high - low) * unit
    }
}
