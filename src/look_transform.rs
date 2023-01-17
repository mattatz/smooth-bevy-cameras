use bevy::{
    app::prelude::*,
    ecs::{bundle::Bundle, prelude::*},
    math::prelude::*,
    prelude::{OrthographicProjection, Projection},
    transform::components::Transform,
};

pub struct LookTransformPlugin;

#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemLabel)]
pub struct LookTransformSystem;

impl Plugin for LookTransformPlugin {
    fn build(&self, app: &mut App) {
        app.add_system(look_transform_system.label(LookTransformSystem))
            .add_system(apply_look_transform_scale_orthographic.after(LookTransformSystem));
    }
}

#[derive(Bundle)]
pub struct LookTransformBundle {
    pub transform: LookTransform,
    pub smoother: Smoother,
}

/// An eye and the target it's looking at. As a component, this can be modified in place of bevy's `Transform`, and the two will
/// stay in sync.
#[derive(Clone, Component, Copy, Debug)]
pub struct LookTransform {
    pub eye: Vec3,
    pub target: Vec3,
    pub up: Option<Vec3>,
    pub scale: Option<f32>,
}

impl From<LookTransform> for Transform {
    fn from(t: LookTransform) -> Self {
        eye_look_at_target_transform(t.eye, t.target, t.up.unwrap_or(Vec3::Y))
    }
}

impl LookTransform {
    pub fn new(eye: Vec3, target: Vec3) -> Self {
        Self {
            eye,
            target,
            up: None,
            scale: None,
        }
    }

    pub fn new_with_scale(eye: Vec3, target: Vec3, scale: f32) -> Self {
        Self {
            eye,
            target,
            up: None,
            scale: Some(scale),
        }
    }

    pub fn radius(&self) -> f32 {
        (self.target - self.eye).length()
    }

    pub fn look_direction(&self) -> Option<Vec3> {
        (self.target - self.eye).try_normalize()
    }
}

fn eye_look_at_target_transform(eye: Vec3, target: Vec3, up: Vec3) -> Transform {
    // If eye and target are very close, we avoid imprecision issues by keeping the look vector a unit vector.
    let look_vector = (target - eye).normalize();
    let look_at = eye + look_vector;

    Transform::from_translation(eye).looking_at(look_at, up)
}

/// Preforms exponential smoothing on a `LookTransform`. Set the `lag_weight` between `0.0` and `1.0`, where higher is smoother.
#[derive(Component)]
pub struct Smoother {
    lag_weight: f32,
    lerp_tfm: Option<LookTransform>,
    enabled: bool,
}

impl Smoother {
    pub fn new(lag_weight: f32) -> Self {
        Self {
            lag_weight,
            lerp_tfm: None,
            enabled: true,
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn current_lerp_tfm(&self) -> &Option<LookTransform> {
        &self.lerp_tfm
    }

    pub(crate) fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if self.enabled {
            // To prevent camera jumping from last lerp before disabling to the current position,
            // reset smoother state
            self.reset();
        }
    }

    pub fn set_lag_weight(&mut self, lag_weight: f32) {
        self.lag_weight = lag_weight;
    }

    pub fn smooth_transform(&mut self, new_tfm: &LookTransform) -> LookTransform {
        debug_assert!(0.0 <= self.lag_weight);
        debug_assert!(self.lag_weight < 1.0);

        let old_lerp_tfm = self.lerp_tfm.unwrap_or(*new_tfm);

        let lead_weight = 1.0 - self.lag_weight;

        let scale = match (old_lerp_tfm.scale, new_tfm.scale) {
            (Some(old_scale), Some(new_scale)) => {
                Some(old_scale * self.lag_weight + new_scale * lead_weight)
            }
            _ => None,
        };

        let lerp_tfm = LookTransform {
            eye: old_lerp_tfm.eye * self.lag_weight + new_tfm.eye * lead_weight,
            target: old_lerp_tfm.target * self.lag_weight + new_tfm.target * lead_weight,
            up: new_tfm.up,
            scale,
        };

        self.lerp_tfm = Some(lerp_tfm);

        lerp_tfm
    }

    pub fn reset(&mut self) {
        self.lerp_tfm = None;
    }
}

fn look_transform_system(
    mut cameras: Query<(&LookTransform, &mut Transform, Option<&mut Smoother>)>,
) {
    for (look_transform, mut scene_transform, smoother) in cameras.iter_mut() {
        match smoother {
            Some(mut s) if s.enabled => {
                let tr = s.smooth_transform(look_transform);
                *scene_transform = tr.into()
            }
            _ => (),
        };
    }
}

fn apply_look_transform_scale_orthographic(
    mut cameras: Query<
        (
            &Smoother,
            Option<&mut Projection>,
            Option<&mut OrthographicProjection>,
        ),
        Or<(With<Projection>, With<OrthographicProjection>)>,
    >,
) {
    for (smoother, proj, orth) in cameras.iter_mut() {
        if smoother.is_enabled() {
            smoother
                .current_lerp_tfm()
                .and_then(|latest| latest.scale)
                .map(|scale| {
                    match (proj, orth) {
                        (Some(mut proj), _) => {
                            if let Projection::Orthographic(orth) = proj.as_mut() {
                                orth.scale = scale;
                            }
                        }
                        (_, Some(mut orth)) => {
                            orth.scale = scale;
                        }
                        _ => {}
                    };
                });
        }
    }
}
