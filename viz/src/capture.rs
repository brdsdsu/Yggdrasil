//! High-resolution capture of the current view, saved as a PNG.
//!
//! The obvious implementation — `Screenshot::primary_window()` — reads back the swapchain,
//! so it can only ever produce what the window already holds: the viewport's pixel count,
//! with the egui panels baked in. That is a screenshot, not a figure.
//!
//! Instead a second camera is spawned with the on-screen camera's transform and projection
//! and a [`RenderTarget::Image`] sized to a multiple of the window, and *that* is captured.
//! Two things fall out of it for free. The framing is exactly what the viewport shows,
//! because the aspect ratio is preserved and the projection is a clone. And the UI is
//! absent, because `bevy_egui` draws into the window's own target and never touches this
//! one — leaving the scene, its edges and spokes, and the gizmo overlays (selection ring,
//! AABB wireframes), which render for every view on the default gizmo layer.
//!
//! **PNG** because the subject is line art on a flat background: node spheres, one-pixel
//! edges and wireframes over a uniform field. JPEG's chroma subsampling frays exactly those
//! and would trade fidelity for a size that does not matter here, while PNG's filters
//! compress a flat background well. It is also what every figure pipeline accepts without
//! conversion.
//!
//! The capture costs a few frames of rendering the scene twice, at 1x..8x the window's
//! pixels; at high scale factors on a large scene that is a visible hitch, and it lasts
//! [`CaptureState::WARMUP_FRAMES`] + 2 frames.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use bevy::prelude::*;
use bevy::render::camera::RenderTarget;
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};
use bevy::render::renderer::RenderDevice;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};
use bevy::window::PrimaryWindow;

/// Marks the offscreen camera, so the query for the on-screen one can exclude it.
#[derive(Component)]
pub struct CaptureCamera;

/// Where a capture has got to.
///
/// The offscreen camera cannot be spawned and captured in the same frame: the screenshot
/// is prepared from `RenderAssets<GpuImage>`, and an image asset created this frame has no
/// GPU texture yet — the capture would be skipped with nothing but a log line to say so.
/// So the camera renders for a few frames first, and the image and the camera are both
/// held alive until after the frame the screenshot actually reads.
#[derive(Default)]
enum Phase {
    #[default]
    Idle,
    /// The button was pressed; the target is built on the next tick.
    Requested,
    /// The offscreen camera is rendering into `image`.
    Warmup {
        image: Handle<Image>,
        camera: Entity,
        frames: u32,
    },
    /// The screenshot entity is spawned. Its render pass writes through this frame, so the
    /// camera and image must outlive it by one.
    Capturing {
        image: Handle<Image>,
        camera: Entity,
        frames: u32,
    },
}

/// The capture button's state: what the UI reads and writes.
#[derive(Resource)]
pub struct CaptureState {
    /// Multiplier on the window's physical size. Clamped against the device's maximum
    /// texture dimension when the capture is built.
    pub scale: u32,
    /// Directory captures are written to — the process's working directory.
    pub dir: PathBuf,
    /// Progress or result, shown under the button. `Err` for a failure the user must see;
    /// the alternative is a log line behind a window they are not reading.
    pub message: Option<Result<String, String>>,
    /// The window's physical size, refreshed every tick so the panel can show the pixel
    /// size a capture would produce without querying the window itself.
    pub window_size: UVec2,
    phase: Phase,
}

impl Default for CaptureState {
    fn default() -> Self {
        Self {
            scale: 4,
            dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            message: None,
            window_size: UVec2::ZERO,
            phase: Phase::default(),
        }
    }
}

impl CaptureState {
    /// Frames the offscreen camera renders before the screenshot is taken.
    ///
    /// One would be enough for the GPU texture to exist; two also covers the first frame
    /// after a resize or a layout change, where the camera's view is still settling.
    pub const WARMUP_FRAMES: u32 = 2;

    /// Scale factors offered in the UI.
    pub const SCALES: [u32; 4] = [1, 2, 4, 8];

    /// Asks for a capture. Ignored while one is already running, so a double-click cannot
    /// leave a second offscreen camera behind.
    pub fn request(&mut self) {
        if matches!(self.phase, Phase::Idle) {
            self.phase = Phase::Requested;
            self.message = Some(Ok("capturing…".to_string()));
        }
    }

    /// Whether a capture is in flight, for graying the button out.
    pub fn busy(&self) -> bool {
        !matches!(self.phase, Phase::Idle)
    }

    /// The pixel size `scale` would produce, before the device limit is applied.
    pub fn target_size(&self) -> UVec2 {
        self.window_size * self.scale
    }
}

/// Advances the capture state machine, one step per frame.
///
/// A single system rather than one per phase: the phases are strictly sequential and each
/// needs the same handful of resources, so splitting them would multiply the plumbing
/// without decoupling anything.
pub fn drive_capture(
    mut commands: Commands,
    mut state: ResMut<CaptureState>,
    mut images: ResMut<Assets<Image>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    device: Option<Res<RenderDevice>>,
    main_camera: Query<(&Transform, &Projection), (With<Camera3d>, Without<CaptureCamera>)>,
) {
    if let Ok(window) = windows.get_single() {
        let size = UVec2::new(window.physical_width(), window.physical_height());
        if size != state.window_size {
            state.window_size = size;
        }
    }

    // Taken out so the arms can move the handles and still assign the next phase.
    match std::mem::take(&mut state.phase) {
        Phase::Idle => {}

        Phase::Requested => {
            state.phase = match begin(&mut commands, &mut images, &state, device, &main_camera) {
                Ok(phase) => phase,
                Err(why) => {
                    state.message = Some(Err(why));
                    Phase::Idle
                }
            };
        }

        Phase::Warmup {
            image,
            camera,
            frames,
        } if frames < CaptureState::WARMUP_FRAMES => {
            state.phase = Phase::Warmup {
                image,
                camera,
                frames: frames + 1,
            };
        }

        Phase::Warmup { image, camera, .. } => {
            let path = unique_path(&state.dir);
            // `save_png` runs inside the observer, so the path has to be owned by it.
            let observed_path = path.clone();
            commands.spawn(Screenshot::image(image.clone())).observe(
                move |trigger: Trigger<ScreenshotCaptured>, mut state: ResMut<CaptureState>| {
                    state.message = Some(save_png(&trigger.event().0, &observed_path));
                },
            );
            state.phase = Phase::Capturing {
                image,
                camera,
                frames: 0,
            };
        }

        // One more frame: the screenshot's render pass runs after this system, so tearing
        // the camera down immediately would capture an unrendered target.
        Phase::Capturing {
            image,
            camera,
            frames,
        } if frames < 1 => {
            state.phase = Phase::Capturing {
                image,
                camera,
                frames: frames + 1,
            };
        }

        Phase::Capturing { camera, .. } => {
            commands.entity(camera).despawn();
            // Dropping the last handle here releases the render target; the readback no
            // longer needs it, having been copied into the screenshot's own buffer.
            state.phase = Phase::Idle;
        }
    }
}

/// Builds the offscreen target and camera, returning the phase to enter.
fn begin(
    commands: &mut Commands,
    images: &mut Assets<Image>,
    state: &CaptureState,
    device: Option<Res<RenderDevice>>,
    main_camera: &Query<(&Transform, &Projection), (With<Camera3d>, Without<CaptureCamera>)>,
) -> Result<Phase, String> {
    let Ok((transform, projection)) = main_camera.get_single() else {
        return Err("no camera to capture".to_string());
    };
    if state.window_size.x == 0 || state.window_size.y == 0 {
        return Err("window has no size yet".to_string());
    }

    // A texture wider or taller than the device allows fails to create at all, so the
    // scale is clamped rather than the request refused — 8x on a 4K window asks for
    // 30,720 pixels across, which no consumer GPU will allocate.
    let limit = device
        .map(|d| d.limits().max_texture_dimension_2d)
        .unwrap_or(8192);
    let longest = state.window_size.x.max(state.window_size.y).max(1);
    let scale = state.scale.min((limit / longest).max(1));
    let size = state.window_size * scale;

    let mut image = Image::new_fill(
        Extent3d {
            width: size.x,
            height: size.y,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 255],
        TextureFormat::Rgba8UnormSrgb,
        // Render world only: nothing reads these pixels on the CPU — the screenshot copies
        // them out of the GPU texture — and at 8x the main-world copy alone is hundreds of
        // megabytes.
        RenderAssetUsages::RENDER_WORLD,
    );
    image.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING
        | TextureUsages::COPY_DST
        | TextureUsages::COPY_SRC
        | TextureUsages::RENDER_ATTACHMENT;
    let image = images.add(image);

    let camera = commands
        .spawn((
            Camera3d::default(),
            Camera {
                target: RenderTarget::Image(image.clone()),
                // Before the window camera, so a frame that does both ends on screen.
                order: -1,
                ..default()
            },
            // Cloned, not rebuilt: a fresh projection would use the default field of view
            // and the capture would no longer frame what the viewport frames.
            projection.clone(),
            *transform,
            CaptureCamera,
        ))
        .id();

    Ok(Phase::Warmup {
        image,
        camera,
        frames: 0,
    })
}

/// Writes the captured image out, returning the message to show.
fn save_png(captured: &Image, path: &Path) -> Result<String, String> {
    let size = UVec2::new(captured.width(), captured.height());
    let dynamic = captured
        .clone()
        .try_into_dynamic()
        .map_err(|e| format!("unsupported capture format: {e}"))?;
    // The alpha channel carries brightness rather than opacity once HDR is on, so an RGBA
    // PNG would come out with holes in it. Bevy's own `save_to_disk` drops it for the same
    // reason.
    dynamic
        .to_rgb8()
        .save_with_format(path, image::ImageFormat::Png)
        .map_err(|e| format!("cannot write {}: {e}", path.display()))?;

    Ok(format!("saved {} ({}x{})", path.display(), size.x, size.y))
}

/// A path in `dir` that no file occupies yet.
///
/// Seconds since the epoch name the file, which sorts chronologically and needs no date
/// formatting dependency; the counter covers two captures inside the same second.
fn unique_path(dir: &Path) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let candidate = dir.join(format!("viz-{stamp}.png"));
    if !candidate.exists() {
        return candidate;
    }
    for n in 1.. {
        let candidate = dir.join(format!("viz-{stamp}-{n}.png"));
        if !candidate.exists() {
            return candidate;
        }
    }
    unreachable!("the loop returns on the first free name")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_request_is_ignored_while_one_is_in_flight() {
        let mut state = CaptureState::default();
        state.request();
        assert!(state.busy());

        // Pretend the capture has advanced past the request.
        state.phase = Phase::Warmup {
            image: Handle::default(),
            camera: Entity::PLACEHOLDER,
            frames: 0,
        };
        state.message = None;
        state.request();

        // Still mid-capture, and the second press left no trace.
        assert!(matches!(state.phase, Phase::Warmup { .. }));
        assert!(state.message.is_none());
    }

    #[test]
    fn target_size_multiplies_the_window() {
        let mut state = CaptureState::default();
        state.window_size = UVec2::new(1920, 1080);
        state.scale = 4;
        assert_eq!(state.target_size(), UVec2::new(7680, 4320));
    }

    #[test]
    fn unique_path_does_not_collide_with_an_existing_capture() {
        let dir = std::env::temp_dir().join(format!("viz-capture-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let first = unique_path(&dir);
        std::fs::write(&first, b"").unwrap();
        let second = unique_path(&dir);

        assert_ne!(first, second);
        assert!(!second.exists());

        std::fs::remove_dir_all(&dir).ok();
    }
}
