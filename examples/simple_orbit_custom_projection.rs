use bevy::{
    core_pipeline::{core_3d, tonemapping::Tonemapping},
    prelude::*,
    render::{
        camera::{
            camera_system, CameraProjection, CameraProjectionPlugin, CameraRenderGraph, ScalingMode,
        },
        primitives::Frustum,
        render_resource::Face,
        view::{update_frusta, VisibleEntities},
    },
    transform::TransformSystem,
};
use smooth_bevy_cameras::{
    controllers::orbit::{OrbitCameraBundle, OrbitCameraController, OrbitCameraPlugin},
    LookTransformPlugin, LookTransformSet, Smoother,
};

fn main() {
    App::new()
        .insert_resource(Msaa::Sample4)
        .add_plugins(DefaultPlugins)
        .add_plugin(CeilingProjectionPlugin)
        .add_plugin(LookTransformPlugin)
        .add_plugin(OrbitCameraPlugin::default())
        .add_startup_system(setup)
        .add_system(apply_look_transform_scale_custom_projection.after(LookTransformSet))
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
        mesh: meshes.add(Mesh::from(shape::Plane {
            size: 5.0,
            subdivisions: 5,
        })),
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
                mouse_rotate_sensitivity: Vec2::new(-0.1, 0.1),
                mouse_translate_sensitivity: Vec2::new(-0.1, 0.1),
                ..Default::default()
            },
            Vec3::new(-2.0, 5.0, 5.0),
            Vec3::new(0., 0., 0.),
            Vec3::Y,
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

#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemSet)]
struct CameraUpdateSystem;

impl Plugin for CeilingProjectionPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(CameraProjectionPlugin::<CeilingProjection>::default())
            .configure_set(CameraUpdateSystem.in_base_set(CoreSet::PostUpdate))
            .add_system(
                update_frusta::<CeilingProjection>
                    .in_set(CameraUpdateSystem)
                    .after(camera_system::<CeilingProjection>)
                    .after(TransformSystem::TransformPropagate),
            );
    }
}

#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component, Default)]
struct CeilingProjection {
    pub near: f32,
    pub far: f32,
    pub viewport_origin: Vec2,
    pub scaling_mode: ScalingMode,
    pub scale: f32,
    pub area: Rect,
}

impl CameraProjection for CeilingProjection {
    fn get_projection_matrix(&self) -> Mat4 {
        Mat4::orthographic_rh(
            // invert x axis
            self.area.max.x,
            self.area.min.x,
            self.area.min.y,
            self.area.max.y,
            // NOTE: near and far are swapped to invert the depth range from [0,1] to [1,0]
            // This is for interoperability with pipelines using infinite reverse perspective projections.
            self.far,
            self.near,
        )
    }

    fn update(&mut self, width: f32, height: f32) {
        let (projection_width, projection_height) = match self.scaling_mode {
            ScalingMode::WindowSize(pixel_scale) => (width / pixel_scale, height / pixel_scale),
            ScalingMode::AutoMin {
                min_width,
                min_height,
            } => {
                // Compare Pixels of current width and minimal height and Pixels of minimal width with current height.
                // Then use bigger (min_height when true) as what it refers to (height when true) and calculate rest so it can't get under minimum.
                if width * min_height > min_width * height {
                    (width * min_height / height, min_height)
                } else {
                    (min_width, height * min_width / width)
                }
            }
            ScalingMode::AutoMax {
                max_width,
                max_height,
            } => {
                // Compare Pixels of current width and maximal height and Pixels of maximal width with current height.
                // Then use smaller (max_height when true) as what it refers to (height when true) and calculate rest so it can't get over maximum.
                if width * max_height < max_width * height {
                    (width * max_height / height, max_height)
                } else {
                    (max_width, height * max_width / width)
                }
            }
            ScalingMode::FixedVertical(viewport_height) => {
                (width * viewport_height / height, viewport_height)
            }
            ScalingMode::FixedHorizontal(viewport_width) => {
                (viewport_width, height * viewport_width / width)
            }
            ScalingMode::Fixed { width, height } => (width, height),
        };

        let origin_x = projection_width * self.viewport_origin.x;
        let origin_y = projection_height * self.viewport_origin.y;
        self.area = Rect::new(
            self.scale * -origin_x,
            self.scale * -origin_y,
            self.scale * (projection_width - origin_x),
            self.scale * (projection_height - origin_y),
        );
    }

    fn far(&self) -> f32 {
        self.far
    }
}

impl Default for CeilingProjection {
    fn default() -> Self {
        CeilingProjection {
            near: 0.0,
            far: 1000.0,
            viewport_origin: Vec2::new(0.5, 0.5),
            scaling_mode: ScalingMode::WindowSize(1.),
            scale: 1.0,
            area: Rect::new(-1.0, -1.0, 1.0, 1.0),
        }
    }
}
