use glam::Vec3;

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

#[cfg(test)]
mod test {
    use super::*;
    use glam::Vec3;
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
