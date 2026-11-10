//! Rendu GPU de Capri, avec wgpu.
//!
//! La surface couvre toute la fenêtre overlay et reste transparente : seuls Capri et, quand il y
//! en a un, son bandeau sont dessinés. Les deux passent par le même shader ; le texte du bandeau
//! est tracé à part, sur le processeur (voir [`banner`]).
//! Sur Windows, la transparence passe par DirectComposition (DirectX 12 en mode « visual ») :
//! le swapchain classique, lié directement à la fenêtre, ne sait pas être transparent.

pub mod banner;

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt as _;

#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("impossible de créer la surface de rendu : {0}")]
    Surface(#[from] wgpu::CreateSurfaceError),
    #[error("aucune carte graphique compatible : {0}")]
    Adapter(#[from] wgpu::RequestAdapterError),
    #[error("impossible d'ouvrir la carte graphique : {0}")]
    Device(#[from] wgpu::RequestDeviceError),
    #[error("le sprite fait {width}×{height} mais ses pixels ne font que {len} octets")]
    BadSprite { width: u32, height: u32, len: usize },
}

/// Image du sprite, en RGBA 8 bits **non** prémultiplié (comme dans un PNG).
#[derive(Clone, Copy, Debug)]
pub struct SpriteImage<'a> {
    pub width: u32,
    pub height: u32,
    pub rgba: &'a [u8],
}

/// Ce qu'on dessine à une image donnée (pixels physiques de la surface).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpriteDraw {
    pub min: [f32; 2],
    pub size: [f32; 2],
    pub ghost: [f32; 2],
    pub boil_seed: u32,
    /// Amplitude du boil, en pixels.
    pub boil: f32,
    pub opacity: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Uniforms {
    viewport: [f32; 2],
    rect_min: [f32; 2],
    rect_size: [f32; 2],
    ghost: [f32; 2],
    seed: f32,
    boil: f32,
    opacity: f32,
    _pad: f32,
}

pub struct Renderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    view_format: wgpu::TextureFormat,
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    sprite: Layer,
    banner: Option<Layer>,
    transparent: bool,
}

/// Une image à dessiner et ses réglages : Capri, ou son bandeau.
struct Layer {
    bind_group: wgpu::BindGroup,
    uniforms: wgpu::Buffer,
}

impl Renderer {
    /// `target` : la fenêtre (un `Arc<winit::window::Window>` convient) ; `width`/`height` :
    /// sa taille en pixels physiques.
    pub fn new(
        target: impl Into<wgpu::SurfaceTarget<'static>>,
        width: u32,
        height: u32,
        sprite: SpriteImage<'_>,
    ) -> Result<Self, RenderError> {
        check_image(sprite)?;

        let instance = wgpu::Instance::new(instance_descriptor());
        let surface = instance.create_surface(target)?;
        // Elle tourne toute la journée : la puce graphique intégrée suffit et ménage la batterie.
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
            apply_limit_buckets: false,
        }))?;
        log::info!("carte graphique : {:?}", adapter.get_info());

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("capri"),
            // la surface fait la taille de l'écran : il faut les limites de textures de la carte
            required_limits: wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits()),
            memory_hints: wgpu::MemoryHints::MemoryUsage,
            ..Default::default()
        }))?;

        let caps = surface.get_capabilities(&adapter);
        // Un format non-sRGB laisse passer les couleurs telles quelles, comme un canvas web :
        // c'est ce qu'il faut pour des images d'anime déjà encodées en sRGB.
        let format = caps.formats.iter().copied().find(|f| !f.is_srgb()).unwrap_or(caps.formats[0]);
        let view_format = format.remove_srgb_suffix();
        let alpha_mode = pick_alpha_mode(&caps.alpha_modes);
        let transparent = alpha_mode != wgpu::CompositeAlphaMode::Opaque;
        if !transparent {
            log::warn!("surface opaque (modes proposés : {:?}) : le fond sera noir", caps.alpha_modes);
        }
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: width.max(1),
            height: height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode,
            // sRGB standard : pas de HDR ni de gamut étendu pour des images d'anime
            color_space: wgpu::SurfaceColorSpace::Auto,
            view_formats: if view_format == format { vec![] } else { vec![view_format] },
        };
        surface.configure(&device, &config);

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("capri sprite"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("capri"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let sprite = Layer::new(&device, &queue, &layout, &sampler, sprite);

        let shader = device.create_shader_module(wgpu::include_wgsl!("sprite.wgsl"));
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("capri"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("capri sprite"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: view_format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        Ok(Self {
            surface,
            device,
            queue,
            config,
            view_format,
            pipeline,
            layout,
            sampler,
            sprite,
            banner: None,
            transparent,
        })
    }

    /// Remplace l'image du bandeau, ou le retire avec `None`.
    pub fn set_banner(&mut self, image: Option<SpriteImage<'_>>) -> Result<(), RenderError> {
        self.banner = match image {
            Some(image) => {
                check_image(image)?;
                Some(Layer::new(&self.device, &self.queue, &self.layout, &self.sampler, image))
            }
            None => None,
        };
        Ok(())
    }

    /// La surface sait-elle être transparente ? (Sinon, le fond est noir.)
    pub fn is_transparent(&self) -> bool {
        self.transparent
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 || (width, height) == (self.config.width, self.config.height) {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
    }

    /// Dessine une image : Capri, puis son bandeau s'il y en a un et que `banner` dit où.
    /// `pre_present` est appelé juste avant l'affichage (winit le demande pour bien caler les
    /// images sur l'écran).
    pub fn render(&mut self, sprite: &SpriteDraw, banner: Option<&SpriteDraw>, pre_present: impl FnOnce()) {
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(f) | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => return,
            other => {
                log::debug!("surface à reconfigurer : {other:?}");
                self.surface.configure(&self.device, &self.config);
                return;
            }
        };

        let viewport = [self.config.width as f32, self.config.height as f32];
        let layers: Vec<(&Layer, &SpriteDraw)> =
            std::iter::once((&self.sprite, sprite)).chain(self.banner.as_ref().zip(banner)).collect();
        for (layer, draw) in &layers {
            let u = Uniforms {
                viewport,
                rect_min: draw.min,
                rect_size: draw.size,
                ghost: draw.ghost,
                seed: draw.boil_seed as f32,
                boil: draw.boil,
                opacity: draw.opacity,
                _pad: 0.0,
            };
            self.queue.write_buffer(&layer.uniforms, 0, bytemuck::bytes_of(&u));
        }

        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(self.view_format),
            ..Default::default()
        });
        let mut encoder =
            self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("capri") });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("capri"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            for (layer, _) in &layers {
                pass.set_bind_group(0, &layer.bind_group, &[]);
                pass.draw(0..6, 0..1);
            }
        }
        self.queue.submit([encoder.finish()]);
        pre_present();
        self.queue.present(frame);
    }
}

impl Layer {
    fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        image: SpriteImage<'_>,
    ) -> Self {
        let texture = upload_sprite(device, queue, image);
        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let uniforms = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("capri uniforms"),
            contents: bytemuck::bytes_of(&Uniforms::zeroed()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("capri"),
            layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: uniforms.as_entire_binding() },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(sampler) },
            ],
        });
        Self { bind_group, uniforms }
    }
}

/// Les pixels font-ils bien la taille annoncée ?
fn check_image(image: SpriteImage<'_>) -> Result<(), RenderError> {
    let expected = image.width as usize * image.height as usize * 4;
    if image.width == 0 || image.height == 0 || image.rgba.len() != expected {
        return Err(RenderError::BadSprite {
            width: image.width,
            height: image.height,
            len: image.rgba.len(),
        });
    }
    Ok(())
}

fn instance_descriptor() -> wgpu::InstanceDescriptor {
    #[allow(unused_mut)]
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    #[cfg(target_os = "macos")]
    {
        desc.backends = wgpu::Backends::METAL;
    }
    #[cfg(windows)]
    {
        // Vulkan ne sait pas faire de fenêtre transparente sur Windows : DirectX 12 + DirectComposition.
        desc.backends = wgpu::Backends::DX12;
        desc.backend_options.dx12.presentation_system = wgpu::Dx12SwapchainKind::DxgiFromVisual;
    }
    desc
}

/// Du plus au moins souhaitable. Metal ne propose que `PostMultiplied` pour une couche non
/// opaque, mais Core Animation compose en prémultiplié : nos couleurs prémultipliées sont justes.
fn pick_alpha_mode(modes: &[wgpu::CompositeAlphaMode]) -> wgpu::CompositeAlphaMode {
    use wgpu::CompositeAlphaMode as M;
    [M::PreMultiplied, M::PostMultiplied, M::Inherit]
        .into_iter()
        .find(|m| modes.contains(m))
        .unwrap_or(M::Opaque)
}

/// Envoie le sprite au GPU, prémultiplié (sinon ses bords auraient un liseré sombre).
fn upload_sprite(device: &wgpu::Device, queue: &wgpu::Queue, sprite: SpriteImage<'_>) -> wgpu::Texture {
    let size = wgpu::Extent3d { width: sprite.width, height: sprite.height, depth_or_array_layers: 1 };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("capri sprite"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &premultiply(sprite.rgba),
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4 * sprite.width),
            rows_per_image: Some(sprite.height),
        },
        size,
    );
    texture
}

fn premultiply(rgba: &[u8]) -> Vec<u8> {
    let mut out = rgba.to_vec();
    for px in out.as_chunks_mut::<4>().0 {
        let a = u16::from(px[3]);
        for c in &mut px[..3] {
            *c = ((u16::from(*c) * a + 127) / 255) as u8;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn premultiplies_colors_by_alpha() {
        assert_eq!(
            premultiply(&[200, 100, 50, 255, 200, 100, 50, 0, 255, 255, 255, 128]),
            vec![200, 100, 50, 255, 0, 0, 0, 0, 128, 128, 128, 128]
        );
    }

    #[test]
    fn prefers_a_transparent_alpha_mode() {
        use wgpu::CompositeAlphaMode as M;
        assert_eq!(pick_alpha_mode(&[M::Opaque, M::PostMultiplied]), M::PostMultiplied); // Metal
        assert_eq!(pick_alpha_mode(&[M::Auto, M::Opaque, M::PreMultiplied]), M::PreMultiplied); // DComp
        assert_eq!(pick_alpha_mode(&[M::Opaque]), M::Opaque);
    }

    #[test]
    fn images_must_have_the_size_they_claim() {
        let image = |width, height, len: usize| SpriteImage { width, height, rgba: &[0u8; 64][..len] };
        assert!(check_image(image(4, 4, 64)).is_ok());
        assert!(matches!(check_image(image(4, 4, 60)), Err(RenderError::BadSprite { len: 60, .. })));
        assert!(check_image(image(0, 4, 0)).is_err(), "une image vide ne se dessine pas");
    }

    #[test]
    fn uniforms_match_the_wgsl_layout() {
        assert_eq!(std::mem::size_of::<Uniforms>(), 48);
    }
}

#[cfg(test)]
mod shader_tests {
    /// Le shader est validé ici, pas seulement au lancement sur la machine de l'utilisateur.
    #[test]
    fn wgsl_parses_and_validates() {
        let module = naga::front::wgsl::parse_str(include_str!("sprite.wgsl")).expect("WGSL valide");
        naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::empty())
            .validate(&module)
            .expect("shader valide pour wgpu");
    }
}
