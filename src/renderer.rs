use std::{error::Error, sync::Arc};

use bytemuck::{Pod, Zeroable};
use glyphon::{
    Attrs, Buffer, Cache, Color, Family, FontSystem, Metrics, Resolution, Shaping, SwashCache,
    TextArea, TextAtlas, TextBounds, TextRenderer, Viewport, Weight, Wrap, cosmic_text::Align,
};
use ttyrantula::ui::{Quad, Scene, TextAlign, TextWeight};
use wgpu::{
    BlendState, ColorTargetState, ColorWrites, CommandEncoderDescriptor, DeviceDescriptor,
    FragmentState, Instance, InstanceDescriptor, LoadOp, MultisampleState, Operations,
    PipelineCompilationOptions, PrimitiveState, RenderPassColorAttachment, RenderPassDescriptor,
    RenderPipeline, RenderPipelineDescriptor, RequestAdapterOptions, Surface, SurfaceConfiguration,
    TextureFormat, TextureViewDescriptor, VertexAttribute, VertexBufferLayout, VertexState,
    VertexStepMode,
    util::{BufferInitDescriptor, DeviceExt},
};
use winit::{dpi::PhysicalSize, event_loop::ActiveEventLoop, window::Window};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Vertex {
    position: [f32; 2],
    color: [f32; 4],
}

impl Vertex {
    const ATTRIBUTES: [VertexAttribute; 2] =
        wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x4];

    fn layout() -> VertexBufferLayout<'static> {
        VertexBufferLayout {
            array_stride: size_of::<Self>() as u64,
            step_mode: VertexStepMode::Vertex,
            attributes: &Self::ATTRIBUTES,
        }
    }
}

pub struct Renderer {
    _instance: Instance,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: Surface<'static>,
    surface_config: SurfaceConfiguration,
    quad_pipeline: RenderPipeline,
    font_system: FontSystem,
    swash_cache: SwashCache,
    viewport: Viewport,
    atlas: TextAtlas,
    text_renderer: TextRenderer,
    window: Arc<Window>,
}

impl Renderer {
    pub async fn new(
        window: Arc<Window>,
        event_loop: &ActiveEventLoop,
    ) -> Result<Self, Box<dyn Error>> {
        let size = window.inner_size();
        let instance = Instance::new(InstanceDescriptor::new_with_display_handle(Box::new(
            event_loop.owned_display_handle(),
        )));
        let surface = instance.create_surface(window.clone())?;
        let adapter = instance
            .request_adapter(&RequestAdapterOptions {
                compatible_surface: Some(&surface),
                ..RequestAdapterOptions::default()
            })
            .await?;
        let (device, queue) = adapter.request_device(&DeviceDescriptor::default()).await?;
        let surface_config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .ok_or("the graphics adapter cannot present to this window")?;
        surface.configure(&device, &surface_config);

        let quad_pipeline = create_quad_pipeline(&device, surface_config.format);
        let font_system = FontSystem::new();
        let swash_cache = SwashCache::new();
        let cache = Cache::new(&device);
        let viewport = Viewport::new(&device, &cache);
        let mut atlas = TextAtlas::new(&device, &queue, &cache, surface_config.format);
        let text_renderer =
            TextRenderer::new(&mut atlas, &device, MultisampleState::default(), None);

        Ok(Self {
            _instance: instance,
            device,
            queue,
            surface,
            surface_config,
            quad_pipeline,
            font_system,
            swash_cache,
            viewport,
            atlas,
            text_renderer,
            window,
        })
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    pub fn resize(&mut self, size: PhysicalSize<u32>) {
        if size.width == 0 || size.height == 0 {
            return;
        }

        self.surface_config.width = size.width;
        self.surface_config.height = size.height;
        self.surface.configure(&self.device, &self.surface_config);
    }

    pub fn render(&mut self, scene: &Scene) -> Result<(), Box<dyn Error>> {
        if self.surface_config.width == 0 || self.surface_config.height == 0 {
            return Ok(());
        }

        self.viewport.update(
            &self.queue,
            Resolution {
                width: self.surface_config.width,
                height: self.surface_config.height,
            },
        );

        let vertices = scene
            .quads
            .iter()
            .flat_map(|quad| {
                quad_vertices(quad, self.surface_config.width, self.surface_config.height)
            })
            .collect::<Vec<_>>();
        let vertex_buffer = self.device.create_buffer_init(&BufferInitDescriptor {
            label: Some("ttyrantula quad vertices"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let text_buffers = scene
            .text
            .iter()
            .map(|primitive| {
                let mut buffer = Buffer::new(
                    &mut self.font_system,
                    Metrics::new(primitive.font_size, primitive.line_height),
                );
                buffer.set_size(Some(primitive.bounds.width), Some(primitive.bounds.height));
                buffer.set_wrap(Wrap::None);
                let weight = match primitive.weight {
                    TextWeight::Normal => Weight::NORMAL,
                    TextWeight::Semibold => Weight::SEMIBOLD,
                };
                buffer.set_text(
                    &primitive.content,
                    &Attrs::new().family(Family::Monospace).weight(weight),
                    Shaping::Advanced,
                    Some(match primitive.align {
                        TextAlign::Left => Align::Left,
                        TextAlign::Right => Align::Right,
                    }),
                );
                buffer.shape_until_scroll(&mut self.font_system, false);
                buffer
            })
            .collect::<Vec<_>>();
        let text_areas = scene
            .text
            .iter()
            .zip(&text_buffers)
            .map(|(primitive, buffer)| TextArea {
                buffer,
                left: primitive.bounds.x,
                top: primitive.bounds.y,
                scale: 1.0,
                bounds: TextBounds {
                    left: primitive.clip_bounds.x.floor() as i32,
                    top: primitive.clip_bounds.y.floor() as i32,
                    right: (primitive.clip_bounds.x + primitive.clip_bounds.width).ceil() as i32,
                    bottom: (primitive.clip_bounds.y + primitive.clip_bounds.height).ceil() as i32,
                },
                default_color: Color::rgba(
                    primitive.color[0],
                    primitive.color[1],
                    primitive.color[2],
                    primitive.color[3],
                ),
                custom_glyphs: &[],
            })
            .collect::<Vec<_>>();

        self.text_renderer.prepare(
            &self.device,
            &self.queue,
            &mut self.font_system,
            &mut self.atlas,
            &self.viewport,
            text_areas,
            &mut self.swash_cache,
        )?;

        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.surface_config);
                self.window.request_redraw();
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                self.window.request_redraw();
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Lost => return Err("graphics surface was lost".into()),
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err("graphics surface validation failed".into());
            }
        };
        let view = frame.texture.create_view(&TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("ttyrantula frame encoder"),
            });

        {
            let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("ttyrantula render pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.quad_pipeline);
            pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            pass.draw(0..vertices.len() as u32, 0..1);
            self.text_renderer
                .render(&self.atlas, &self.viewport, &mut pass)?;
        }

        self.queue.submit(Some(encoder.finish()));
        self.queue.present(frame);
        self.atlas.trim();
        Ok(())
    }
}

fn create_quad_pipeline(device: &wgpu::Device, format: TextureFormat) -> RenderPipeline {
    let shader = device.create_shader_module(wgpu::include_wgsl!("shader.wgsl"));
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("ttyrantula quad pipeline layout"),
        bind_group_layouts: &[],
        immediate_size: 0,
    });

    device.create_render_pipeline(&RenderPipelineDescriptor {
        label: Some("ttyrantula quad pipeline"),
        layout: Some(&layout),
        vertex: VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: PipelineCompilationOptions::default(),
            buffers: &[Some(Vertex::layout())],
        },
        primitive: PrimitiveState::default(),
        depth_stencil: None,
        multisample: MultisampleState::default(),
        fragment: Some(FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: PipelineCompilationOptions::default(),
            targets: &[Some(ColorTargetState {
                format,
                blend: Some(BlendState::ALPHA_BLENDING),
                write_mask: ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}

fn quad_vertices(quad: &Quad, width: u32, height: u32) -> [Vertex; 6] {
    let left = quad.bounds.x / width as f32 * 2.0 - 1.0;
    let right = (quad.bounds.x + quad.bounds.width) / width as f32 * 2.0 - 1.0;
    let top = 1.0 - quad.bounds.y / height as f32 * 2.0;
    let bottom = 1.0 - (quad.bounds.y + quad.bounds.height) / height as f32 * 2.0;
    let vertex = |position| Vertex {
        position,
        color: quad.color,
    };

    [
        vertex([left, top]),
        vertex([left, bottom]),
        vertex([right, bottom]),
        vertex([left, top]),
        vertex([right, bottom]),
        vertex([right, top]),
    ]
}
