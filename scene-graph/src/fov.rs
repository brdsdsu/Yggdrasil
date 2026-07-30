use glam::Vec3;

use spatial_index::{RegionMembershipPredicate, RegionMembershipStatus};

use crate::error::{Result, SceneGraphError};

/// Observer represent a Field-of-View cone frustum in 3D space.
/// The cone is defined by a position, a viewing direction, and a half-angle (in radians). the
/// frustum is further limited by near and far distances.
///
///  ↑↑            O <---------- observer position
///  ||           /|\
///  ||          / | \   
///  ||near     /  |  \  
///  ||        /   |-θ-\ <------ half_angle
///  ||       /         \
///  |↓      *-----------* <---- Chord with `near` radius
///  |      / observable  \
///  |far  /    volume     \
///  ↓    *-----------------* <- Chord with `far` radius
///
#[derive(Clone, Copy, Debug)]
pub struct Observer {
    /// Position of the observer/camera in world space.
    position: Vec3,

    /// Unit vector the observer/camera looks along, in world space.
    /// A cone is rotationally symmetric about this axis.
    direction: Vec3,

    /// `half_angle` represents the maximum angle (in radians) from the forward
    /// direction that is still considered "inside" the field of view.
    half_angle_cos: f32,

    /// Near distance of the frustum. Points closer than this are not
    /// observed by the observer.
    near: f32,

    /// Far distance of the frustum. Points farther than this are not
    /// observed by the observer.
    far: f32,
}

impl Observer {
    /// Build an observer looking along `direction`, in whatever frame the caller's
    /// coordinates already use.
    ///
    /// `half_angle` is in radians. Returns error if `direction` is not finite or has
    /// no length, since neither yields a viewing axis.
    ///
    /// ```rust
    /// # use scene_graph::Observer;
    /// # use glam::Vec3;
    /// // A Z-up scene: look horizontally along +X.
    /// let observer = Observer::new_looking_along(
    ///     Vec3::ZERO,
    ///     Vec3::X,
    ///     35_f32.to_radians(),
    ///     0.3,
    ///     8.0,
    /// )
    /// .unwrap();
    ///
    /// assert!(observer.observers(&Vec3::new(5.0, 0.0, 0.0)));
    /// // Straight up is outside a 35-degree cone pointed along +X.
    /// assert!(!observer.observers(&Vec3::new(0.0, 0.0, 5.0)));
    /// ```
    pub fn new_looking_along(
        pos: Vec3,
        direction: Vec3,
        half_angle: f32,
        near: f32,
        far: f32,
    ) -> Result<Self> {
        let Some(forward) = direction.try_normalize() else {
            return Err(SceneGraphError::ObserverCreationError);
        };
        Ok(Self {
            position: pos,
            direction: forward,
            half_angle_cos: half_angle.cos(),
            near,
            far,
        })
    }

    /// Cone-frustum membership test.
    pub fn observers(&self, p: &Vec3) -> bool {
        // vector from observer to point
        let v = p - self.position;
        // reachability test
        let d = v.length();
        if d < self.near || d > self.far || d == 0.0 {
            return false;
        }
        let dir = v / d;
        let cos_theta = dir.dot(self.direction); // both unit
        cos_theta >= self.half_angle_cos
    }
}

/// The observer *is* the region predicate — there is no separate cone type, so a caller passes
/// `&observer` wherever a [`RegionMembershipPredicate`] is wanted.
///
/// [`Self::contains`] delegates to [`Observer::observers`] rather than restating it, which makes
/// the two agree by construction instead of by test. That matters: a query pruned by `classify`
/// and filtered by `contains` must answer exactly the question `observers` defines, or it is a
/// different query that happens to be fast.
impl RegionMembershipPredicate<f32, 3> for Observer {
    /// Reject a box only when the whole of it lies outside the frustum.
    ///
    /// Two independent rejections, either sufficient:
    ///
    /// * **Range.** The box's nearest point to the apex is beyond `far`, or its farthest point is
    ///   nearer than `near`. Both are exact for a box.
    /// * **Angle.** In the (axial, perpendicular) cross-section the cone's lateral surface is the
    ///   ray from the apex along `(cos θ, sin θ)`, whose outward normal is `(-sin θ, cos θ)`. So
    ///   `perp · cos θ - axial · sin θ` is the signed distance from the box's centre to that
    ///   surface, positive outside. Exceeding the box's bounding-sphere radius puts the whole
    ///   sphere, and therefore the whole box, beyond it.
    ///
    /// The angular form is exact for a sphere and conservative for a box — a long thin cell is
    /// bounded by a sphere much larger than itself, so some rejectable cells come back
    /// `Undecided`. That direction is safe. It is also correct behind the apex, where a negative
    /// `axial` only makes the term larger, and for a half-angle past 90 degrees, where `cos θ`
    /// goes negative and the boundary tilts backwards without changing the sign convention.
    fn classify(&self, lo: &[f32; 3], hi: &[f32; 3]) -> RegionMembershipStatus {
        let apex = self.position;

        // Nearest and farthest squared distances from the apex to the box. The nearest is the
        // usual per-axis clamp; the farthest takes whichever side of each axis is further away.
        let (mut nearest_sq, mut farthest_sq) = (0.0f32, 0.0f32);
        for axis in 0..3 {
            let below = lo[axis] - apex[axis];
            let above = apex[axis] - hi[axis];
            let outside = below.max(above).max(0.0);
            nearest_sq += outside * outside;
            let farthest = below.abs().max(above.abs());
            farthest_sq += farthest * farthest;
        }
        if nearest_sq > self.far * self.far || farthest_sq < self.near * self.near {
            return RegionMembershipStatus::Reject;
        }

        let centre = Vec3::new(
            0.5 * (lo[0] + hi[0]),
            0.5 * (lo[1] + hi[1]),
            0.5 * (lo[2] + hi[2]),
        );
        let half = Vec3::new(
            0.5 * (hi[0] - lo[0]),
            0.5 * (hi[1] - lo[1]),
            0.5 * (hi[2] - lo[2]),
        );
        let radius = half.length();

        let v = centre - apex;
        let axial = v.dot(self.direction);
        // Clamped because catastrophic cancellation can push this a hair below zero for a point
        // on the axis, and `sqrt` of a negative is NaN — which would compare false and silently
        // turn a rejection into `Undecided`.
        let perp = (v.length_squared() - axial * axial).max(0.0).sqrt();

        // `sin θ` from the stored cosine. Exact for any half-angle in `[0, π]`, where sine is
        // non-negative, so the observer needs no extra field and nothing is recovered via `acos`.
        let sin_theta = (1.0 - self.half_angle_cos * self.half_angle_cos)
            .max(0.0)
            .sqrt();

        if perp * self.half_angle_cos - axial * sin_theta > radius {
            return RegionMembershipStatus::Reject;
        }

        RegionMembershipStatus::Undecided
    }

    #[inline]
    fn contains(&self, point: &[f32; 3]) -> bool {
        self.observers(&(*point).into())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use glam::Vec3;

    /// The conservative-rejection contract, checked with the harness that ships beside the
    /// trait rather than a bespoke test — the point of putting it in `spatial-index` is that
    /// every implementor gets checked the same way.
    ///
    /// A wrong `Reject` is invisible downstream: the query simply returns fewer results. So
    /// this is the only thing standing between a subtly wrong cone and silently missing
    /// geometry.
    #[test]
    fn classify_never_rejects_a_box_holding_a_visible_point() {
        let observer =
            Observer::new_looking_along(Vec3::ZERO, Vec3::X, 35_f32.to_radians(), 0.3, 8.0)
                .unwrap();
        let with_members = spatial_index::test_utils::region::assert_conservative(
            &observer,
            [-12.0, 12.0],
            0.05..=3.0,
            20_000,
            0x5eed_1234,
        );
        assert!(
            with_members > 100,
            "only {with_members} boxes held a visible point, so the run barely exercised the \
             property it exists to check"
        );
    }

    /// Wide cones are the case the sign convention could plausibly get wrong: past 90 degrees
    /// `cos` goes negative and the lateral surface tilts backwards. The volume also stops being
    /// convex there, which does not affect rejection but would affect an `Inside` verdict if one
    /// were ever added.
    #[test]
    fn classify_stays_conservative_for_a_cone_wider_than_a_hemisphere() {
        let observer =
            Observer::new_looking_along(Vec3::ZERO, Vec3::X, 120_f32.to_radians(), 0.3, 8.0)
                .unwrap();
        let with_members = spatial_index::test_utils::region::assert_conservative(
            &observer,
            [-12.0, 12.0],
            0.05..=3.0,
            20_000,
            0xfeed_4321,
        );
        assert!(with_members > 100, "cone too wide to be exercised: {with_members}");
    }

    /// `contains` delegates to `observers`, so this guards the delegation rather than the maths.
    #[test]
    fn contains_agrees_with_observers() {
        let observer =
            Observer::new_looking_along(Vec3::ZERO, Vec3::X, 35_f32.to_radians(), 0.3, 8.0)
                .unwrap();
        spatial_index::test_utils::region::assert_contains_matches(
            &observer,
            |p| observer.observers(&Vec3::from_array(*p)),
            [-12.0, 12.0],
            50_000,
            0xabcd_ef01,
        );
    }

    #[test]
    fn classify_rejects_boxes_outside_the_frustum() {
        use spatial_index::RegionMembershipStatus::*;
        let observer =
            Observer::new_looking_along(Vec3::ZERO, Vec3::X, 35_f32.to_radians(), 0.3, 8.0)
                .unwrap();
        let cube = |c: [f32; 3], h: f32| {
            (
                [c[0] - h, c[1] - h, c[2] - h],
                [c[0] + h, c[1] + h, c[2] + h],
            )
        };

        for (name, centre, half) in [
            ("beyond the far plane", [40.0, 0.0, 0.0], 1.0),
            ("off the viewing axis", [0.0, 5.0, 0.0], 0.5),
            ("behind the apex", [-5.0, 0.0, 0.0], 1.0),
        ] {
            let (lo, hi) = cube(centre, half);
            assert_eq!(observer.classify(&lo, &hi), Reject, "should reject: {name}");
        }

        // Sitting on the axis inside the range: cannot be ruled out.
        let (lo, hi) = cube([4.0, 0.0, 0.0], 1.0);
        assert_eq!(observer.classify(&lo, &hi), Undecided);
    }
    /// A Z-up scene, panning horizontally.+X is a pan here and the cone follows it as a given.
    #[test]
    fn looking_along_is_frame_agnostic() {
        let half_angle = 35_f32.to_radians();
        let observer = Observer::new_looking_along(Vec3::ZERO, Vec3::X, half_angle, 0.3, 8.0).unwrap();

        assert!(
            observer.observers(&Vec3::new(5.0, 0.0, 0.0)),
            "straight ahead"
        );
        assert!(
            !observer.observers(&Vec3::new(0.0, 0.0, 5.0)),
            "straight up"
        );
        assert!(!observer.observers(&Vec3::new(-5.0, 0.0, 0.0)), "behind");
        assert!(
            !observer.observers(&Vec3::new(0.1, 0.0, 0.0)),
            "nearer than near"
        );
        assert!(!observer.observers(&Vec3::new(9.0, 0.0, 0.0)), "beyond far");
    }

    /// Only the axis matters, so directions that differ merely in length describe
    /// the same cone.
    #[test]
    fn direction_length_does_not_change_membership() {
        let half_angle = 35_f32.to_radians();
        let points = [
            Vec3::new(0.0, 0.0, 2.0),
            Vec3::new(1.0, 0.0, 2.0),
            Vec3::new(0.0, 1.0, 2.0),
            Vec3::new(3.0, 3.0, 1.0),
        ];
        let reference = Observer::new_looking_along(Vec3::ZERO, Vec3::Z, half_angle, 0.6, 6.0).unwrap();
        for scale in [0.01_f32, 1.0, 500.0] {
            let scaled =
                Observer::new_looking_along(Vec3::ZERO, Vec3::Z * scale, half_angle, 0.6, 6.0).unwrap();
            for point in points {
                assert_eq!(
                    scaled.observers(&point),
                    reference.observers(&point),
                    "scale {scale} changed membership at {point:?}"
                );
            }
        }
    }

    #[test]
    fn looking_along_rejects_directions_with_no_axis() {
        let half_angle = 35_f32.to_radians();
        assert!(Observer::new_looking_along(Vec3::ZERO, Vec3::ZERO, half_angle, 0.3, 8.0).is_err());
        assert!(
            Observer::new_looking_along(
                Vec3::ZERO,
                Vec3::new(f32::NAN, 0.0, 0.0),
                half_angle,
                0.3,
                8.0
            )
            .is_err()
        );
        assert!(
            Observer::new_looking_along(
                Vec3::ZERO,
                Vec3::new(f32::INFINITY, 0.0, 0.0),
                half_angle,
                0.3,
                8.0
            )
            .is_err()
        );
    }

    /// The stored axis is the direction handed in, not something reconstructed from
    /// an intermediate rotation, so it must come back bit-for-bit once normalized.
    #[test]
    fn looking_along_stores_the_direction_it_was_given() {
        let half_angle = 35_f32.to_radians();
        for direction in [
            Vec3::X,
            Vec3::NEG_Z,
            Vec3::new(1.0, 2.0, -3.0),
            Vec3::new(0.0, -4.0, 0.0),
        ] {
            let observer =
                Observer::new_looking_along(Vec3::ZERO, direction, half_angle, 0.3, 8.0).unwrap();
            assert_eq!(observer.direction, direction.normalize(), "{direction:?}");
        }
    }

    #[test]
    fn cone_frustum_check() {
        // Observer at origin, looking mostly along +Z, panned 30° toward +X and
        // tilted 5° down.
        let pos = Vec3::new(0.0, 0.0, 0.0);
        let direction = Vec3::new(0.4981, -0.0872, 0.8627);

        // Cone View Frustum: half-angle=35°, near=0.6, far=6.0
        let half_angle = 35_f32.to_radians();
        let near = 0.6;
        let far = 6.0;

        let cone = Observer::new_looking_along(pos, direction, half_angle, near, far).unwrap();

        // Some test points (world space)
        let pts = [
            (Vec3::new(2.0, 0.4, 4.5), true),    // likely inside
            (Vec3::new(0.2, 0.1, 0.4), false), // inside angle but closer than near (should be false)
            (Vec3::new(5.0, 2.5, 0.5), false), // near edge
            (Vec3::new(-1.0, 0.0, 2.0), false), // behind-ish/side (likely outside angle)
            (Vec3::new(20.0, 0.0, 30.0), false), // beyond far (false)
        ];

        for (p, is_observable) in pts.iter() {
            assert_eq!(cone.observers(p), *is_observable);
        }
    }

    #[test]
    fn cone_frustum_along_z() {
        // Observer at origin, looking straight down +Z.
        let pos = Vec3::new(0.0, 0.0, 0.0);

        // Cone View Frustum: half-angle=35°, near=0.6, far=6.0
        let half_angle = 35_f32.to_radians();
        let near = 0.6;
        let far = 6.0;

        let cone = Observer::new_looking_along(pos, Vec3::Z, half_angle, near, far).unwrap();
        assert!(cone.observers(&Vec3::new(0.0, 0.0, 1.0)));
        assert!(!cone.observers(&Vec3::new(6.0, 6.0, 6.0)));
    }
}
