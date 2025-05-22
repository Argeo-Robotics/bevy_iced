use bevy_derive::{Deref, DerefMut};
use bevy_ecs::prelude::*;
use bevy_render::render_graph::RenderLabel;
use bevy_render::{
    Extract,
    render_graph::{Node, NodeRunError, RenderGraphContext},
    renderer::RenderContext,
    view::ExtractedWindows,
};
use bevy_window::prelude::*;
use cfg_if::cfg_if;
use iced_core::Size;
use iced_wgpu::wgpu::TextureFormat;
use iced_widget::graphics::Viewport;

use crate::{DidDraw, IcedProps, IcedResource, IcedSettings};

#[derive(Clone, Hash, Debug, Eq, PartialEq, RenderLabel)]
pub struct IcedPass;

pub const TEXTURE_FMT: TextureFormat = TextureFormat::Bgra8UnormSrgb;

#[derive(Resource, Deref, DerefMut, Clone)]
pub struct IcedViewport(pub Viewport);

pub fn update_viewport(
    windows: Query<&Window>,
    iced_settings: Res<IcedSettings>,
    mut commands: Commands,
) {
    let window = windows.single().unwrap();
    let scale_factor = iced_settings
        .scale_factor
        .unwrap_or_else(|| window.scale_factor().into());
    let viewport = Viewport::with_physical_size(
        Size::new(window.physical_width(), window.physical_height()),
        scale_factor,
    );
    commands.insert_resource(IcedViewport(viewport));
}

// Same as DidDraw, but as a regular bool instead of an atomic.
#[derive(Resource, Deref, DerefMut)]
struct DidDrawBasic(bool);

pub fn extract_iced_data(
    mut commands: Commands,
    viewport: Extract<Res<IcedViewport>>,
    did_draw: Extract<Res<DidDraw>>,
) {
    commands.insert_resource(viewport.clone());
    commands.insert_resource(DidDrawBasic(
        did_draw.swap(false, std::sync::atomic::Ordering::Relaxed),
    ));
}

pub fn recall_staging_belt(
    #[cfg(target_arch = "wasm32")] iced: NonSend<IcedResource>,
    #[cfg(not(target_arch = "wasm32"))] iced: Res<IcedResource>,
) {
    iced.lock().renderer.staging_belt_recall();
}

pub struct IcedNode;

impl Node for IcedNode {
    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        world: &World,
    ) -> Result<(), NodeRunError> {
        let Some(extracted_window) = world
            .get_resource::<ExtractedWindows>()
            .unwrap()
            .windows
            .values()
            .next()
        else {
            return Ok(());
        };

        cfg_if! {
            if #[cfg(target_arch = "wasm32")] {
                let IcedProps {
                    renderer,
                    ..
                } = &mut *world.non_send_resource::<IcedResource>().lock();
            } else {
                let IcedProps {
                    renderer,
                    ..
                } = &mut *world.resource::<IcedResource>().lock();
            }
        };
        let viewport = world.resource::<IcedViewport>();

        if !world.get_resource::<DidDrawBasic>().is_some_and(|x| x.0) {
            return Ok(());
        }

        if let Some(view) = &extracted_window.swap_chain_texture_view {
            let encoder = renderer.draw(None, view, viewport);
            render_context.add_command_buffer(encoder.finish());
        }
        renderer.staging_belt_finish();

        Ok(())
    }
}
