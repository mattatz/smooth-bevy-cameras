use bevy::{
    core_pipeline::{core_3d, tonemapping::Tonemapping},
    prelude::*,
    render::{
        camera::{
            camera_system, CameraProjection, CameraProjectionPlugin, CameraRenderGraph,
            ScalingMode, WindowOrigin,
        },
        primitives::Frustum,
        view::{update_frusta, VisibleEntities}, render_resource::Face,
    },
};
use smooth_bevy_cameras::{
    controllers::orbit::{OrbitCameraBundle, OrbitCameraController, OrbitCameraPlugin},
    LookTransformPlugin, LookTransformSystem, Smoother,
};

fn main() {
    App::new()
        .insert_resource(Msaa { samples: 4 })
        .add_plugins(DefaultPlugins)
        .add_plugin(CeilingProjectionPlugin)
        .add_plugin(LookTransformPlugin)
        .add_plugin(OrbitCameraPlugin::default())
        .add_startup_system(setup)
        .add_system(apply_look_transform_scale_custom_projection.after(LookTransformSystem))
        .run();
}

/// set up a simple 3D scene
fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // plane
    let mut material: StandardMaterial = Color::rgb(0.3, 0.5, 0.3).into();
    material.cull_mode = Some(Face::Front);
    commands.spawn(PbrBundle {
        mesh: meshes.add(Mesh::from(shape::Plane { size: 5.0 })),
        material: materials.add(material),
        ..Default::default()
    });

    // cube
    let mut material: StandardMaterial = Color::rgb(0.8, 0.7, 0.6).into();
    material.cull_mode = Some(Face::Front);
    commands.spawn(PbrBundle {
        mesh: meshes.add(Mesh::from(shape::Cube { size: 1.0 })),
        material: materials.add(material),
        transform: Transform::from_xyz(0.0, 0.5, 0.0),
        ..Default::default()
    });

    // light
    commands.spawn(PointLightBundle {
        transform: Transform::from_xyz(4.0, 8.0, 4.0),
        ..Default::default()
    });

    let projection = CeilingProjection {
        scale: 5.0,
        scaling_mode: ScalingMode::FixedVertical(2.0),
        ..Default::default()
    };

    let mut transform = Transform::from_translation(Vec3::new(-2.0, 5.0, 5.0));
    transform.look_at(Vec3::new(0., 0., 0.), Vec3::Y);
    let custom_camera_bundle = (
        Camera::default(),
        projection,
        CameraRenderGraph::new(core_3d::graph::NAME),
        Camera3d::default(),
        transform,
        GlobalTransform::default(),
        VisibleEntities::default(),
        Frustum::default(),
        Tonemapping::default(),
    );

    commands
        .spawn(OrbitCameraBundle::new_with_scale(
            OrbitCameraController {
                mouse_rotate_sensitivity: Vec2::new(-0.006, 0.006),
                mouse_translate_sensitivity: Vec2::new(-0.008, 0.008),
                ..Default::default()
            },
            Vec3::new(-2.0, 5.0, 5.0),
            Vec3::new(0., 0., 0.),
            5.,
        ))
        .insert(custom_camera_bundle);
}

fn apply_look_transform_scale_custom_projection(
    mut cameras: Query<(&Smoother, &mut CeilingProjection)>,
) {
    for (smoother, mut proj) in cameras.iter_mut() {
        if smoother.is_enabled() {
            smoother
                .current_lerp_tfm()
                .and_then(|latest| latest.scale)
                .map(|scale| {
                    proj.scale = scale;
                });
        }
    }
}

struct CeilingProjectionPlugin;

#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemLabel)]
struct UpdateProjectionFrusta;

impl Plugin for CeilingProjectionPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(CameraProjectionPlugin::<CeilingProjection>::default())
            .add_system_to_stage(
                CoreStage::PostUpdate,
                update_frusta::<CeilingProjection>
                    .label(UpdateProjectionFrusta)
                    .after(camera_system::<CeilingProjection>),
            );
    }
}

#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component, Default)]
struct CeilingProjection {
    pub left: f32,
    pub right: f32,
    pub bottom: f32,
    pub top: f32,
    pub near: f32,
    pub far: f32,
    pub window_origin: WindowOrigin,
    pub scaling_mode: ScalingMode,
    pub scale: f32,
}

impl CameraProjection for CeilingProjection {
    fn get_projection_matrix(&self) -> Mat4 {
        Mat4::orthographic_rh(
            // invert x axis
            -self.left * self.scale,
            -self.right * self.scale,
            self.bottom * self.scale,
            self.top * self.scale,
            // NOTE: near and far are swapped to invert the depth range from [0,1] to [1,0]
            // This is for interoperability with pipelines using infinite reverse perspective projections.
            self.far,
            self.near,
        )
    }

    fn update(&mut self, width: f32, height: f32) {
        let (viewport_width, viewport_height) = match self.scaling_mode {
            ScalingMode::WindowSize => (width, height),
            ScalingMode::Auto {
                min_width,
                min_height,
            } => {
                if width * min_height > min_width * height {
                    (width * min_height / height, min_height)
                } else {
                    (min_width, height * min_width / width)
                }
            }
            ScalingMode::FixedVertical(viewport_height) => {
                (width * viewport_height / height, viewport_height)
            }
            ScalingMode::FixedHorizontal(viewport_width) => {
                (viewport_width, height * viewport_width / width)
            }
            ScalingMode::None => return,
        };

        match self.window_origin {
            WindowOrigin::Center => {
                let half_width = viewport_width / 2.0;
                let half_height = viewport_height / 2.0;
                self.left = -half_width;
                self.bottom = -half_height;
                self.right = half_width;
                self.top = half_height;

                if let ScalingMode::WindowSize = self.scaling_mode {
                    if self.scale == 1.0 {
                        self.left = self.left.floor();
                        self.bottom = self.bottom.floor();
                        self.right = self.right.floor();
                        self.top = self.top.floor();
                    }
                }
            }
            WindowOrigin::BottomLeft => {
                self.left = 0.0;
                self.bottom = 0.0;
                self.right = viewport_width;
                self.top = viewport_height;
            }
        }
    }

    fn far(&self) -> f32 {
        self.far
    }
}

impl Default for CeilingProjection {
    fn default() -> Self {
        CeilingProjection {
            left: -1.0,
            right: 1.0,
            bottom: -1.0,
            top: 1.0,
            near: 0.0,
            far: 1000.0,
            window_origin: WindowOrigin::Center,
            scaling_mode: ScalingMode::WindowSize,
            scale: 1.0,
        }
    }
}
