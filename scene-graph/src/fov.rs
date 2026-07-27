use glam::{Quat, Vec3};

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
    /// let observer = Observer::looking_along(
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
    pub fn looking_along(
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

    /// Build from yaw/pitch/roll (radians) in a right-handed XYZ system:
    /// yaw: +Y, pitch: +X, roll: +Z. Rotation order: yaw * pitch * roll.
    /// half_angle: radians.
    /// near, far: distances.
    /// yaw, pitch, and roll: radians.
    ///
    /// Note that composing the rotation this way assumes a **Y-up** frame: the
    /// forward axis starts at local +Z and yaw turns about +Y. In a Z-up scene that
    /// makes "yaw" tilt the view rather than pan it, so prefer
    /// [`looking_along`](Observer::looking_along) there.
    ///
    /// `roll` spins the observer about its own viewing axis, which a cone cannot
    /// tell apart from no roll at all, so it is consumed here and not stored.
    pub fn from_ypr(
        pos: Vec3,
        yaw: f32,
        pitch: f32,
        roll: f32,
        half_angle: f32,
        near: f32,
        far: f32,
    ) -> Self {
        let r_yaw = Quat::from_rotation_y(yaw);
        let r_pitch = Quat::from_rotation_x(pitch);
        let r_roll = Quat::from_rotation_z(roll);
        let rot = r_yaw * r_pitch * r_roll;
        Self {
            position: pos,
            // The rotation only ever reaches the cone through the axis it puts the
            // local forward (+Z) on, so resolve it here and keep the axis alone.
            direction: (rot * Vec3::Z).normalize(),
            half_angle_cos: half_angle.cos(),
            near,
            far,
        }
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
    /// The case `from_ypr` cannot express naturally: a Z-up scene, panning
    /// horizontally. Under `from_ypr` a yaw turns about +Y, which in Z-up is a
    /// horizontal axis, so the view tilts instead of panning.
    #[test]
    fn looking_along_is_frame_agnostic() {
        let half_angle = 35_f32.to_radians();
        let observer = Observer::looking_along(Vec3::ZERO, Vec3::X, half_angle, 0.3, 8.0).unwrap();

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

    /// A direction that is already the local forward axis must agree with the
    /// equivalent `from_ypr` call, so the two constructors cannot drift apart.
    #[test]
    fn looking_along_z_matches_unrotated_ypr() {
        let half_angle = 35_f32.to_radians();
        let by_direction =
            Observer::looking_along(Vec3::ZERO, Vec3::Z, half_angle, 0.6, 6.0).unwrap();
        let by_angles = Observer::from_ypr(Vec3::ZERO, 0.0, 0.0, 0.0, half_angle, 0.6, 6.0);

        for point in [
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(1.0, 1.0, 3.0),
            Vec3::new(6.0, 6.0, 6.0),
            Vec3::new(0.0, 0.0, -2.0),
            Vec3::new(2.0, 0.0, 2.0),
        ] {
            assert_eq!(
                by_direction.observers(&point),
                by_angles.observers(&point),
                "constructors disagree at {point:?}"
            );
        }
    }

    /// The cone is rotationally symmetric about its axis, so no roll can change
    /// membership -- which is why `looking_along` needs no roll parameter.
    #[test]
    fn roll_cannot_change_membership() {
        let half_angle = 35_f32.to_radians();
        let points = [
            Vec3::new(0.0, 0.0, 2.0),
            Vec3::new(1.0, 0.0, 2.0),
            Vec3::new(0.0, 1.0, 2.0),
            Vec3::new(3.0, 3.0, 1.0),
        ];
        let reference = Observer::from_ypr(Vec3::ZERO, 0.0, 0.0, 0.0, half_angle, 0.6, 6.0);
        for roll in [0.5_f32, 1.0, 2.0, -1.5] {
            let rolled = Observer::from_ypr(Vec3::ZERO, 0.0, 0.0, roll, half_angle, 0.6, 6.0);
            for point in points {
                assert_eq!(
                    rolled.observers(&point),
                    reference.observers(&point),
                    "roll {roll} changed membership at {point:?}"
                );
            }
        }
    }

    #[test]
    fn looking_along_rejects_directions_with_no_axis() {
        let half_angle = 35_f32.to_radians();
        assert!(Observer::looking_along(Vec3::ZERO, Vec3::ZERO, half_angle, 0.3, 8.0).is_err());
        assert!(
            Observer::looking_along(
                Vec3::ZERO,
                Vec3::new(f32::NAN, 0.0, 0.0),
                half_angle,
                0.3,
                8.0
            )
            .is_err()
        );
        assert!(
            Observer::looking_along(
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
                Observer::looking_along(Vec3::ZERO, direction, half_angle, 0.3, 8.0).unwrap();
            assert_eq!(observer.direction, direction.normalize(), "{direction:?}");
        }
    }

    #[test]
    fn cone_frustum_check() {
        // Observer at origin, yaw=30°, pitch=5°, roll=0°
        let pos = Vec3::new(0.0, 0.0, 0.0);
        let yaw = 30_f32.to_radians();
        let pitch = 5_f32.to_radians();
        let roll = 0.0;

        // Cone View Frustum: half-angle=35°, near=0.6, far=6.0
        let half_angle = 35_f32.to_radians();
        let near = 0.6;
        let far = 6.0;

        let cone = Observer::from_ypr(pos, yaw, pitch, roll, half_angle, near, far);

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
    fn cone_frustum_no_rotation() {
        // Observer at origin, yaw=30°, pitch=5°, roll=0°
        let pos = Vec3::new(0.0, 0.0, 0.0);
        let yaw = 0_f32.to_radians();
        let pitch = 0_f32.to_radians();
        let roll = 0_f32.to_radians();

        // Cone View Frustum: half-angle=35°, near=0.6, far=6.0
        let half_angle = 35_f32.to_radians();
        let near = 0.6;
        let far = 6.0;

        let cone = Observer::from_ypr(pos, yaw, pitch, roll, half_angle, near, far);
        assert!(cone.observers(&Vec3::new(0.0, 0.0, 1.0)));
        assert!(!cone.observers(&Vec3::new(6.0, 6.0, 6.0)));
    }
}
