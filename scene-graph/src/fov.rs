use glam::{Quat, Vec3};

/// Observer represent a Field-of-View cone frustum in 3D space.
/// The cone is defined by a position, orientation (quaternion),
/// and a half-angle (in radians). The frustum is further limited
/// by near and far distances.
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

    /// Orientation of the observer/camera as a quaternion.
    rotation: Quat,

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
    /// Build from yaw/pitch/roll (radians) in a right-handed XYZ system:
    /// yaw: +Y, pitch: +X, roll: +Z. Rotation order: yaw * pitch * roll.
    /// half_angle: radians.
    /// near, far: distances.
    /// yaw, pitch, and roll: radians.
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
            rotation: rot,
            half_angle_cos: half_angle.cos(),
            near,
            far,
        }
    }

    /// Forward vector in world space (+Z is forward in local frame).
    #[inline]
    fn forward(&self) -> Vec3 {
        (self.rotation * Vec3::Z).normalize()
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
        let cos_theta = dir.dot(self.forward()); // both unit
        cos_theta >= self.half_angle_cos
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use glam::Vec3;
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
