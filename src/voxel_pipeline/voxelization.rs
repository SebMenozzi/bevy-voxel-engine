use super::voxel_world::{VoxelData, VoxelUniforms};
use crate::{Flags, RenderGraphSettings, VOXELS_PER_METER};

use bevy::{
    asset::{load_internal_asset, Handle},
    core_pipeline::{core_3d::Transparent3d},
    ecs::system::{
        lifetimeless::{Read, SQuery, SRes},
        SystemParamItem,
    },
    pbr::{
        DrawMesh, MeshPipeline, MeshPipelineKey, RenderMeshInstances, SetMeshBindGroup,
        SetMeshViewBindGroup,
    },
    prelude::*,
    render::{
        Render, RenderApp, RenderSet,
        camera::{RenderTarget, ScalingMode, ClearColorConfig},
        extract_component::{ExtractComponent, ExtractComponentPlugin},
        mesh::MeshVertexBufferLayout,
        render_asset::RenderAssets,
        render_phase::{
            AddRenderCommand, DrawFunctions, PhaseItem, RenderCommand, RenderCommandResult,
            RenderPhase, SetItemPipeline, TrackedRenderPass,
        },
        render_resource::*,
        renderer::{RenderDevice, RenderQueue},
        texture::FallbackImage,
        view::ExtractedView,
    },
    utils::HashMap,
};

const VOXELIZATION_SHADER_HANDLE: Handle<Shader> = Handle::weak_from_u128(1975691635883203525);

pub struct VoxelizationPlugin;

impl Plugin for VoxelizationPlugin {

    fn build(&self, app: &mut App) {
        load_internal_asset!(
            app,
            VOXELIZATION_SHADER_HANDLE,
            "shaders/voxelization.wgsl",
            Shader::from_wgsl
        );

        app.add_plugins(ExtractComponentPlugin::<VoxelizationMaterial>::default())
            .add_systems(Startup, setup)
            .add_systems(Update, update_cameras);
    }

    fn finish(&self, app: &mut App) {
        app
            .sub_app_mut(RenderApp)
            .add_render_command::<Transparent3d, DrawCustom>()
            .init_resource::<VoxelizationPipeline>()
            .init_resource::<SpecializedMeshPipelines<VoxelizationPipeline>>()
            .insert_resource(VoxelizationUniformsResource(HashMap::new()))
            .add_systems(
                Render,
                (
                    queue_custom.in_set(RenderSet::QueueMeshes),
                    queue_bind_group.in_set(RenderSet::Queue),
                ),
            );
    }
}

#[derive(Resource, Deref, DerefMut)]
struct VoxelizationImage(Handle<Image>);

#[derive(Component)]
struct VoxelizationCamera;

fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    // image that is the size of the render world to create the correct ammount of fragments
    let size = Extent3d {
        width: 1,
        height: 1,
        ..default()
    };
    let image = Image {
        texture_descriptor: TextureDescriptor {
            label: None,
            size,
            dimension: TextureDimension::D2,
            format: TextureFormat::Bgra8UnormSrgb,
            mip_level_count: 1,
            sample_count: 1,
            usage: TextureUsages::TEXTURE_BINDING
                | TextureUsages::COPY_DST
                | TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[TextureFormat::Bgra8UnormSrgb],
        },
        ..default()
    };
    let image_handle = images.add(image);
    commands.insert_resource(VoxelizationImage(image_handle.clone()));

    // priorities of -3, -2 and -1 so that they are rendered before the main pass
    for i in 0..3 {
        let camera = commands.spawn((Camera3dBundle {
            camera: Camera {
                target: RenderTarget::Image(image_handle.clone()),
                order: -3 + i,
                clear_color: ClearColorConfig::None,
                ..default()
            },
            main_texture_usages: Default::default(),
            camera_3d: Camera3d::default(),
            ..default()
        }, VoxelizationCamera)).id();

        commands.spawn(TargetCamera(camera));
    }
}

fn update_cameras(
    voxelization_image: Res<VoxelizationImage>,
    mut images: ResMut<Assets<Image>>,
    mut voxelization_cameras: Query<(&mut Transform, &mut Projection), With<VoxelizationCamera>>,
    voxel_uniforms: Res<VoxelUniforms>,
) {
    let voxelization_image = images
        .get_mut(voxelization_image.id())
        .expect("Voxelization image not found");

    if voxelization_image.size().x as u32 != voxel_uniforms.texture_size {

        // Update cameras
        debug!(
            "Updating {} voxelization cameras to a resolution of {}",
            voxelization_cameras.iter().len(),
            voxel_uniforms.texture_size
        );

        let mut i = 0;
        for (mut transform, mut projection) in voxelization_cameras.iter_mut() {
            // Resize image
            let size = voxel_uniforms.texture_size;
            voxelization_image.resize(Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            });

            // Update camera
            *transform = match i {
                0 => Transform::from_translation(Vec3::ZERO).looking_at(Vec3::X, Vec3::Y),
                1 => Transform::from_translation(Vec3::ZERO).looking_at(Vec3::Y, Vec3::Z),
                2 => Transform::from_translation(Vec3::ZERO).looking_at(Vec3::Z, Vec3::Y),
                _ => panic!("Too many voxelization cameras"),
            };

            let side = size as f32 / VOXELS_PER_METER / 2.0;
            
            *projection = Projection::Orthographic(OrthographicProjection {
                near: -side,
                far: side,
                scaling_mode: ScalingMode::Fixed {
                    width: 2.0 * side,
                    height: 2.0 * side,
                },
                ..default()
            });

            i += 1;
        }
    }
}

#[derive(Component, Clone, ExtractComponent)]
pub struct VoxelizationMaterial {
    pub material: VoxelizationMaterialType,
    pub flags: u8,
}

impl Default for VoxelizationMaterial {
    fn default() -> Self {
        Self {
            material: VoxelizationMaterialType::Material(10),
            flags: Flags::ANIMATION_FLAG,
        }
    }
}

#[derive(Clone)]
pub enum VoxelizationMaterialType {
    Texture(Handle<Image>),
    Material(u8),
}

#[derive(Clone, ShaderType)]
pub struct VoxelizationUniforms {
    material: u32,
    flags: u32,
}

impl From<&VoxelizationMaterial> for VoxelizationUniforms {
    fn from(value: &VoxelizationMaterial) -> Self {
        let material = match &value.material {
            VoxelizationMaterialType::Texture(_) => 255,
            VoxelizationMaterialType::Material(material) => *material as u32,
        };
        Self {
            material,
            flags: value.flags as u32,
        }
    }
}

type DrawCustom = (
    SetItemPipeline,
    SetMeshViewBindGroup<0>,
    SetMeshBindGroup<1>,
    SetVoxelWorldBindGroup<2>,
    SetVoxelizationBindGroup<3>,
    DrawMesh,
);

#[derive(Resource)]
pub struct VoxelizationPipeline {
    mesh_pipeline: MeshPipeline,
    world_bind_group_layout: BindGroupLayout,
    voxelization_bind_group_layout: BindGroupLayout,
}

impl FromWorld for VoxelizationPipeline {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();
        let voxel_world_data = world.resource::<VoxelData>();

        let world_bind_group_layout = voxel_world_data.bind_group_layout.clone();
        let voxelization_bind_group_layout =
            render_device.create_bind_group_layout(
                None,
                &[
                    BindGroupLayoutEntry {
                        binding: 0,
                        visibility: ShaderStages::FRAGMENT,
                        ty: BindingType::Buffer {
                            ty: BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: BufferSize::new(
                                VoxelizationUniforms::SHADER_SIZE.into(),
                            ),
                        },
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 1,
                        visibility: ShaderStages::FRAGMENT,
                        ty: BindingType::Texture {
                            sample_type: TextureSampleType::Float { filterable: false },
                            view_dimension: TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 2,
                        visibility: ShaderStages::FRAGMENT,
                        ty: BindingType::Sampler(SamplerBindingType::NonFiltering),
                        count: None,
                    },
                ],
            );

        VoxelizationPipeline {
            mesh_pipeline: world.resource::<MeshPipeline>().clone(),
            world_bind_group_layout,
            voxelization_bind_group_layout,
        }
    }
}

impl SpecializedMeshPipeline for VoxelizationPipeline {
    type Key = MeshPipelineKey;

    fn specialize(
        &self,
        key: Self::Key,
        layout: &MeshVertexBufferLayout,
    ) -> Result<RenderPipelineDescriptor, SpecializedMeshPipelineError> {
        let mut descriptor = self.mesh_pipeline.specialize(key, layout)?;

        descriptor.vertex.shader = VOXELIZATION_SHADER_HANDLE;
        descriptor.fragment.as_mut().unwrap().shader = VOXELIZATION_SHADER_HANDLE;

        descriptor
            .vertex
            .shader_defs
            .push("MESH_BINDGROUP_1".into());

        descriptor.layout = vec![
            self.mesh_pipeline.get_view_layout(key.into()).clone(),
            self.mesh_pipeline.mesh_layouts.model_only.clone(),
            self.world_bind_group_layout.clone(),
            self.voxelization_bind_group_layout.clone(),
        ];

        descriptor.primitive.cull_mode = None;

        Ok(descriptor)
    }
}

fn queue_custom(
    transparent_3d_draw_functions: Res<DrawFunctions<Transparent3d>>,
    custom_pipeline: Res<VoxelizationPipeline>,
    mut pipelines: ResMut<SpecializedMeshPipelines<VoxelizationPipeline>>,
    mut pipeline_cache: ResMut<PipelineCache>,
    render_meshes: Res<RenderAssets<Mesh>>,
    material_meshes: Query<Entity, With<VoxelizationMaterial>>,
    render_mesh_instances: Res<RenderMeshInstances>,
    mut views: Query<(&ExtractedView, &mut RenderPhase<Transparent3d>)>,
    render_graph_settings: Res<RenderGraphSettings>,
) {
    if !render_graph_settings.voxelization {
        return;
    }

    let draw_custom = transparent_3d_draw_functions
        .read()
        .get_id::<DrawCustom>()
        .unwrap();

    let key = MeshPipelineKey::from_primitive_topology(PrimitiveTopology::TriangleList);

    for (view, mut transparent_phase) in &mut views {
        let rangefinder = view.rangefinder3d();

         for entity in &material_meshes {
            let Some(mesh_instance) = render_mesh_instances.get(&entity) else {
                continue;
            };
            let Some(mesh) = render_meshes.get(mesh_instance.mesh_asset_id) else {
                continue;
            };

            let pipeline = pipelines
                .specialize(&mut pipeline_cache, &custom_pipeline, key, &mesh.layout)
                .unwrap();

            transparent_phase.add(Transparent3d {
                entity,
                pipeline,
                draw_function: draw_custom,
                distance: rangefinder
                    .distance_translation(&mesh_instance.transforms.transform.translation),
                batch_range: 0..1,
                dynamic_offset: None,
            });
        }
    }
}

#[derive(Component, Deref, DerefMut)]
struct VoxelizationBindGroup(BindGroup);

#[derive(Resource, Deref, DerefMut)]
struct VoxelizationUniformsResource(HashMap<Entity, UniformBuffer<VoxelizationUniforms>>);

fn queue_bind_group(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    voxelization_materials: Query<(Entity, &VoxelizationMaterial)>,
    gpu_images: Res<RenderAssets<Image>>,
    voxelization_pipeline: Res<VoxelizationPipeline>,
    fallback_images: Res<FallbackImage>,
    mut voxelization_uniforms: ResMut<VoxelizationUniformsResource>,
) {
    for (entity, voxelization_material) in voxelization_materials.iter() {
        let uniforms = voxelization_uniforms
            .entry(entity)
            .or_insert(UniformBuffer::from(VoxelizationUniforms::from(
                voxelization_material,
            )));

        uniforms.set(voxelization_material.into());
        uniforms.write_buffer(&render_device, &render_queue);

        let sampler = render_device.create_sampler(&SamplerDescriptor::default());

        let image_view =
            if let VoxelizationMaterialType::Texture(texture) = &voxelization_material.material {
                gpu_images.get(texture).unwrap_or(&fallback_images.d2)
            } else {
                &fallback_images.d2
            };

        let voxelization_bind_group = render_device.create_bind_group(
            None,
            &voxelization_pipeline.voxelization_bind_group_layout,
            &[
                BindGroupEntry {
                    binding: 0,
                    resource: uniforms.binding().unwrap(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::TextureView(&image_view.texture_view),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::Sampler(&sampler),
                },
            ],
        );

        commands
            .entity(entity)
            .insert(VoxelizationBindGroup(voxelization_bind_group));
    }

    let mut to_remove = Vec::new();
    for entity in voxelization_uniforms.keys() {
        if voxelization_materials.get(*entity).is_err() {
            to_remove.push(*entity);
        }
    }
    for entity in to_remove {
        voxelization_uniforms.remove(&entity);
    }
}

struct SetVoxelWorldBindGroup<const I: usize>;

impl<P: PhaseItem, const I: usize> RenderCommand<P> for SetVoxelWorldBindGroup<I> {
    type Param = SRes<VoxelData>;
    type ViewQuery = ();
    type ItemQuery = ();

    fn render<'w>(
        _item: &P,
        _view: (),
        _entity: Option<()>,
        query: SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let voxel_world_data = query.into_inner();

        pass.set_bind_group(I, &voxel_world_data.bind_group, &[]);

        RenderCommandResult::Success
    }
}

struct SetVoxelizationBindGroup<const I: usize>;

impl<P: PhaseItem, const I: usize> RenderCommand<P> for SetVoxelizationBindGroup<I> {
    type Param = SQuery<Read<VoxelizationBindGroup>>;
    type ViewQuery = ();
    type ItemQuery = ();

    fn render<'w>(
        item: &P,
        _view: (),
        _entity: Option<()>,
        query: SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let voxelization_bind_group = query.get_inner(item.entity()).unwrap();

        pass.set_bind_group(I, voxelization_bind_group, &[]);

        RenderCommandResult::Success
    }
}
