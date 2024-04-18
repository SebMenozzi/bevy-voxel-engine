use self::{
    attachments::{AttachmentsNode, AttachmentsPlugin},
    compute::{
        animation::AnimationNode, automata::AutomataNode, clear::ClearNode,
        physics::PhysicsNode, rebuild::RebuildNode, ComputeResourcesPlugin,
    },
    trace::{TraceNode, TracePlugin},
    voxel_world::VoxelWorldPlugin,
    voxelization::VoxelizationPlugin,
};
use bevy::{
    core_pipeline::{
        fxaa::FxaaNode, 
        tonemapping::TonemappingNode, 
        upscaling::UpscalingNode,
    },
    prelude::*,
    render::{
        RenderApp,
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        graph::CameraDriverLabel,
        render_graph::{RenderGraph, RenderSubGraph, RenderLabel, ViewNodeRunner},
    },
    ui::UiPassNode,
};

pub mod attachments;
pub mod compute;
pub mod trace;
pub mod voxel_world;
pub mod voxelization;

pub struct RenderPlugin;

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
enum VoxelGraphLabel {
    Attachments,
    Trace,
    //Bloom,
    Tonemapping,
    Fxaa,
    Ui,
    Upscaling,
    Rebuild,
    Physics,
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
enum RenderGraphLabel {
    Clear,
    Automata,
    Animation,
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderSubGraph)]
pub struct VoxelGraph;

impl Plugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(RenderGraphSettings::default())
            .add_plugins(ExtractResourcePlugin::<RenderGraphSettings>::default())
            .add_plugins(AttachmentsPlugin)
            .add_plugins(VoxelWorldPlugin)
            .add_plugins(TracePlugin)
            .add_plugins(VoxelizationPlugin)
            .add_plugins(ComputeResourcesPlugin);
    }

    fn finish(&self, app: &mut App) {
        let render_app = app.sub_app_mut(RenderApp);
        let render_world = &mut render_app.world;

        // Build voxel render graph
        let mut voxel_graph = RenderGraph::default();

        // Voxel render graph
        let attachments = AttachmentsNode::new(render_world);
        let trace = TraceNode::new(render_world);
        //let bloom = BloomNode::new(render_world);
        let tonemapping = TonemappingNode::from_world(render_world);
        let fxaa = FxaaNode::from_world(render_world);
        let ui = UiPassNode::new(render_world);
        let upscaling = UpscalingNode::from_world(render_world);

        voxel_graph.add_node(VoxelGraphLabel::Attachments, attachments);
        voxel_graph.add_node(VoxelGraphLabel::Trace, trace);
        //voxel_graph.add_node(VoxelGraphLabel::Bloom, ViewNodeRunner::new(bloom, render_world));
        voxel_graph.add_node(VoxelGraphLabel::Tonemapping, ViewNodeRunner::new(tonemapping, render_world));
        voxel_graph.add_node(VoxelGraphLabel::Fxaa, ViewNodeRunner::new(fxaa, render_world));
        voxel_graph.add_node(VoxelGraphLabel::Ui, ui);
        voxel_graph.add_node(VoxelGraphLabel::Upscaling, ViewNodeRunner::new(upscaling, render_world));

        voxel_graph.add_node_edge(VoxelGraphLabel::Trace, VoxelGraphLabel::Tonemapping);
        //voxel_graph.add_node_edge(VoxelGraphLabel::Bloom, VoxelGraphLabel::Tonemapping);
        voxel_graph.add_node_edge(VoxelGraphLabel::Tonemapping, VoxelGraphLabel::Fxaa);
        voxel_graph.add_node_edge(VoxelGraphLabel::Fxaa, VoxelGraphLabel::Ui);
        voxel_graph.add_node_edge(VoxelGraphLabel::Ui, VoxelGraphLabel::Upscaling);

        voxel_graph.add_slot_edge(VoxelGraphLabel::Attachments, "normal", VoxelGraphLabel::Trace, "normal");
        voxel_graph.add_slot_edge(VoxelGraphLabel::Attachments, "position", VoxelGraphLabel::Trace, "position");

        // Voxel render graph compute
        voxel_graph.add_node(VoxelGraphLabel::Rebuild, RebuildNode);
        voxel_graph.add_node(VoxelGraphLabel::Physics, PhysicsNode);

        voxel_graph.add_node_edge(VoxelGraphLabel::Rebuild, VoxelGraphLabel::Physics);
        voxel_graph.add_node_edge(VoxelGraphLabel::Physics, VoxelGraphLabel::Trace);

        // Main graph compute
        let mut main_graph = render_world.resource_mut::<RenderGraph>();

        main_graph.add_node(RenderGraphLabel::Clear, ClearNode);
        main_graph.add_node(RenderGraphLabel::Automata, AutomataNode);
        main_graph.add_node(RenderGraphLabel::Animation, AnimationNode);

        main_graph.add_node_edge(RenderGraphLabel::Clear, RenderGraphLabel::Automata);
        main_graph.add_node_edge(RenderGraphLabel::Automata, RenderGraphLabel::Animation);
        main_graph.add_node_edge(RenderGraphLabel::Animation, CameraDriverLabel);

        // Insert the voxel graph into the main render graph
        main_graph.add_sub_graph(VoxelGraph, voxel_graph);
    }
}

#[derive(Resource, Clone, ExtractResource)]
pub struct RenderGraphSettings {
    pub clear: bool,
    pub automata: bool,
    pub animation: bool,
    pub voxelization: bool,
    pub rebuild: bool,
    pub physics: bool,
    pub trace: bool,
}

impl Default for RenderGraphSettings {
    fn default() -> Self {
        Self {
            clear: true,
            automata: true,
            animation: true,
            voxelization: true,
            rebuild: true,
            physics: true,
            trace: true,
        }
    }
}
