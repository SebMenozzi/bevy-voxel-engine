use bevy::{
    core_pipeline::{bloom::BloomSettings, fxaa::Fxaa},
    prelude::*,
};
use bevy_voxel_engine::{
    BevyVoxelEnginePlugin, BoxCollider, Flags, LoadVoxelWorld, VoxelCameraBundle, VoxelPhysics,
    VoxelizationBundle, VoxelizationMaterial, VoxelizationMaterialType,
};
use character::CharacterEntity;

#[path = "common/character.rs"]
mod character;

#[path = "common/fps_counter.rs"]
mod fps_counter;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(BevyVoxelEnginePlugin)
        .add_plugins(character::Character)
        .add_plugins(fps_counter::FpsCounter)
        .add_systems(Startup, setup)
        .add_systems(Update, update)
        .run();
}

#[derive(Component)]
struct SandSpawner;

fn setup(
    mut commands: Commands,
    mut load_voxel_world: ResMut<LoadVoxelWorld>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    // voxel world
    *load_voxel_world = LoadVoxelWorld::File("assets/monu9.vox".to_string());

    let transform = Transform::from_xyz(5.0, 5.0, -5.0).looking_at(Vec3::ZERO, Vec3::Y);

    commands.spawn((
        VoxelCameraBundle {
            transform,
            projection: Projection::Perspective(PerspectiveProjection {
                fov: 1.57,
                ..default()
            }),
            ..default()
        },
        CharacterEntity {
            in_spectator: true,
            grounded: false,
            look_at: -transform.local_z(),
            up: Vec3::new(0.0, 1.0, 0.0),
        },
        VoxelPhysics::new(
            Vec3::splat(0.0),
            Vec3::splat(0.0),
            bevy_voxel_engine::CollisionEffect::None,
        ),
        BoxCollider {
            half_size: IVec3::new(2, 4, 2),
        },
        // supports bloom and fxaa
        BloomSettings::default(),
        Fxaa::default(),
    ));

    // sand spawner
    commands.spawn((
        VoxelizationBundle {
            mesh_handle: meshes.add(Mesh::from(shape::UVSphere {
                radius: 3.0,
                ..default()
            })),
            transform: Transform::from_xyz(5.0, 0.0, 5.0),
            voxelization_material: VoxelizationMaterial {
                flags: Flags::NONE,
                ..default()
            },
            ..default()
        },
        SandSpawner,
    ));
}

fn update(
    mut sand_spawner: Query<(&mut Transform, &mut VoxelizationMaterial), With<SandSpawner>>,
    character_query: Query<&Transform, (With<CharacterEntity>, Without<SandSpawner>)>,
    input: Res<Input<MouseButton>>,
) {
    let character = character_query.single();
    let (mut sand_spawner, mut sand_material) = sand_spawner.single_mut();

    sand_spawner.translation = character.translation - character.local_z() * 10.0;

    if input.pressed(MouseButton::Left) {
        sand_material.material = VoxelizationMaterialType::Material(8);
    } else {
        sand_material.material = VoxelizationMaterialType::Material(0);
    }
}
