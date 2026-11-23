//! Rendu GPU de Capri, avec wgpu.
//!
//! La surface couvre toute la fenêtre overlay et reste transparente : seuls Capri et, quand il y
//! en a un, son bandeau sont dessinés. Les deux passent par le même shader ; le texte du bandeau
//! est tracé à part, sur le processeur (voir [`banner`]).
//!
//! **Ses images** sont confiées une fois pour toutes à la carte graphique ([`Renderer::add_image`]) :
//! son dessin d'avant (une image entière) et ses planches, remises en grilles d'au plus 2 304 px de
//! côté. Chaque image dessinée en est une part rectangulaire ([`Crop`]) : une case de planche, ou
//! l'image entière. Une fois confiée, l'image n'a plus besoin d'être gardée en mémoire.
//!
//! **Une toile** ([`Renderer::add_canvas`]) est une image qu'on réécrit en place, d'une seule
//! texture : l'intro du lancement y peint ses images l'une après l'autre
//! ([`Renderer::paint_canvas`], des pixels déjà prémultipliés par [`premultiply_in_place`], hors du
//! thread principal), sans jamais en garder plus d'une sur la carte graphique. Toute image peut
//! être lâchée ([`Renderer::remove_image`]) : sa texture est libérée, et son numéro ne sert plus.
//!
//! Sur Windows, la transparence passe par DirectComposition (DirectX 12 en mode « visual ») :
//! le swapchain classique, lié directement à la fenêtre, ne sait pas être transparent.
//!
//! [`Offscreen`] dessine de même dans une image en mémoire, sans fenêtre : pour les essais qui
//! veulent regarder ce qui serait affiché.

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
    #[error("l'image fait {width}×{height} mais ses pixels ne font que {len} octets")]
    BadSprite { width: u32, height: u32, len: usize },
    #[error(
        "l'image fait {width}×{height} : la carte graphique n'en accepte pas de plus de {max} px de côté"
    )]
    TooLarge { width: u32, height: u32, max: u32 },
    #[error("l'image rendue n'a pas pu être relue : {0}")]
    Readback(String),
    #[error("l'image n° {0} n'existe pas, ou plus")]
    UnknownImage(usize),
}

/// Image, en RGBA 8 bits **non** prémultiplié (comme dans un PNG).
#[derive(Clone, Copy, Debug)]
pub struct SpriteImage<'a> {
    pub width: u32,
    pub height: u32,
    pub rgba: &'a [u8],
}

/// Une image confiée à la carte graphique.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ImageId(usize);

/// La part d'une image à dessiner, en coordonnées normalisées de l'image (0..1) : son coin haut
/// gauche et sa taille.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Crop {
    pub min: [f32; 2],
    pub size: [f32; 2],
}

impl Crop {
    /// L'image entière.
    pub const WHOLE: Self = Self { min: [0.0, 0.0], size: [1.0, 1.0] };

    /// Le rectangle de `size` pixels dont le coin haut gauche est en `origin`, dans une image de
    /// `image` pixels (une case dans sa grille).
    pub fn pixels(origin: (u32, u32), size: (u32, u32), image: (u32, u32)) -> Self {
        let (w, h) = (image.0.max(1) as f32, image.1.max(1) as f32);
        Self { min: [origin.0 as f32 / w, origin.1 as f32 / h], size: [size.0 as f32 / w, size.1 as f32 / h] }
    }
}

/// Ce qu'on dessine à une image donnée (pixels physiques de la surface).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpriteDraw {
    pub min: [f32; 2],
    pub size: [f32; 2],
    /// Décalage de sa copie violette, en pixels.
    pub ghost: [f32; 2],
    /// Force de sa copie violette : 0, aucune copie.
    pub ghost_strength: f32,
    pub boil_seed: u32,
    /// Amplitude du boil, en pixels.
    pub boil: f32,
    pub opacity: f32,
    /// La part de l'image à dessiner.
    pub crop: Crop,
}

/// La force de la copie violette du dessin d'avant et du bandeau.
pub const GHOST: f32 = 0.55;

/// Ce qu'attend au plus [`Renderer::settle`] : la carte graphique finit en quelques millisecondes
/// ce que Capri lui donne, et la boucle ne doit pas rester bloquée si elle tarde.
pub const SETTLE: std::time::Duration = std::time::Duration::from_millis(50);

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Uniforms {
    viewport: [f32; 2],
    rect_min: [f32; 2],
    rect_size: [f32; 2],
    ghost: [f32; 2],
    crop_min: [f32; 2],
    crop_size: [f32; 2],
    seed: f32,
    boil: f32,
    opacity: f32,
    ghost_strength: f32,
}

impl Uniforms {
    fn of(draw: &SpriteDraw, viewport: [f32; 2]) -> Self {
        Self {
            viewport,
            rect_min: draw.min,
            rect_size: draw.size,
            ghost: draw.ghost,
            crop_min: draw.crop.min,
            crop_size: draw.crop.size,
            seed: draw.boil_seed as f32,
            boil: draw.boil,
            opacity: draw.opacity,
            ghost_strength: draw.ghost_strength,
        }
    }
}

pub struct Renderer {
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    view_format: wgpu::TextureFormat,
    painter: Painter,
    transparent: bool,
}

/// Ce qui dessine, quelle que soit la cible : la carte graphique, le shader, les images confiées.
struct Painter {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    uniforms_layout: wgpu::BindGroupLayout,
    image_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    /// Les images confiées, à la place de leur numéro ; `None` une fois lâchée.
    images: Vec<Option<Stored>>,
    banner: Option<wgpu::BindGroup>,
    /// Un jeu de réglages par dessin de l'image en cours (Capri, sa bouche, son bandeau…),
    /// créés au besoin et gardés d'une image à l'autre.
    slots: Vec<Slot>,
    /// Ce que pèsent les images confiées, en octets de mémoire graphique.
    image_bytes: u64,
}

struct Slot {
    buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

/// Une image confiée : ce que lit le shader, sa texture (pour la réécrire), et sa taille.
struct Stored {
    bind_group: wgpu::BindGroup,
    texture: wgpu::Texture,
    width: u32,
    height: u32,
}

impl Stored {
    fn bytes(&self) -> u64 {
        u64::from(self.width) * u64::from(self.height) * 4
    }
}

impl Renderer {
    /// `target` : la fenêtre (un `Arc<winit::window::Window>` convient) ; `width`/`height` :
    /// sa taille en pixels physiques.
    pub fn new(
        target: impl Into<wgpu::SurfaceTarget<'static>>,
        width: u32,
        height: u32,
    ) -> Result<Self, RenderError> {
        let instance = wgpu::Instance::new(instance_descriptor());
        let surface = instance.create_surface(target)?;
        let (adapter, device, queue) = open(&instance, Some(&surface))?;

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
        let painter = Painter::new(device, queue, view_format);
        Ok(Self { surface, config, view_format, painter, transparent })
    }

    /// Confie une image à la carte graphique : elle peut ensuite être lâchée.
    pub fn add_image(&mut self, image: SpriteImage<'_>) -> Result<ImageId, RenderError> {
        self.painter.add_image(image)
    }

    /// Une toile de `width` × `height` pixels, transparente, à réécrire en place
    /// ([`Renderer::paint_canvas`]).
    pub fn add_canvas(&mut self, width: u32, height: u32) -> Result<ImageId, RenderError> {
        self.painter.add_canvas(width, height)
    }

    /// Réécrit toute la toile `id` : `premultiplied`, RGBA 8 bits **déjà prémultiplié**
    /// ([`premultiply_in_place`]), de sa taille exacte. Visible au prochain dessin.
    pub fn paint_canvas(&mut self, id: ImageId, premultiplied: &[u8]) -> Result<(), RenderError> {
        self.painter.paint_canvas(id, premultiplied)
    }

    /// Lâche l'image `id` : sa texture est libérée (une fois la carte graphique passée), et son
    /// numéro ne sert plus. Sans effet si elle l'est déjà.
    pub fn remove_image(&mut self, id: ImageId) {
        self.painter.remove_image(id);
    }

    /// Attend que la carte graphique ait fini ce qui lui a été soumis ([`SETTLE`] au plus), et rend
    /// tout de suite ce qu'elle ne tient plus : les images lâchées, les copies faites. Sinon, wgpu
    /// ne le fait qu'à la soumission suivante, et une fenêtre cachée n'en fait plus aucune.
    pub fn settle(&self) {
        self.painter.settle();
    }

    /// Ce que pèsent les images confiées, en octets de mémoire graphique (le bandeau à part).
    pub fn image_bytes(&self) -> u64 {
        self.painter.image_bytes
    }

    /// Remplace l'image du bandeau, ou le retire avec `None`.
    pub fn set_banner(&mut self, image: Option<SpriteImage<'_>>) -> Result<(), RenderError> {
        self.painter.set_banner(image)
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
        self.surface.configure(&self.painter.device, &self.config);
    }

    /// Dessine une image : chaque part de `layers` dans l'ordre (Capri, puis sa bouche par-dessus),
    /// puis son bandeau s'il y en a un et que `banner` dit où. `pre_present` est appelé juste avant
    /// l'affichage (winit le demande pour bien caler les images sur l'écran). Rend `false` si rien
    /// n'a été présenté : la fenêtre n'est pas visible (sur macOS, wgpu ne dessine pas dans une
    /// fenêtre que le système dit occultée, y compris juste après qu'elle a été montrée), le
    /// système n'a pas donné d'image à temps, ou la surface était à reconfigurer.
    pub fn render(
        &mut self,
        layers: &[(ImageId, SpriteDraw)],
        banner: Option<&SpriteDraw>,
        pre_present: impl FnOnce(),
    ) -> bool {
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(f) | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => return false,
            other => {
                log::debug!("surface à reconfigurer : {other:?}");
                self.surface.configure(&self.painter.device, &self.config);
                return false;
            }
        };
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(self.view_format),
            ..Default::default()
        });
        let viewport = [self.config.width as f32, self.config.height as f32];
        let commands = self.painter.encode(&view, viewport, layers, banner);
        self.painter.queue.submit([commands]);
        pre_present();
        self.painter.queue.present(frame);
        true
    }
}

impl Painter {
    fn new(device: wgpu::Device, queue: wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("capri sprite"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let uniforms_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("capri réglages"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let image_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("capri image"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let shader = device.create_shader_module(wgpu::include_wgsl!("sprite.wgsl"));
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("capri"),
            bind_group_layouts: &[Some(&uniforms_layout), Some(&image_layout)],
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
                    format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        Self {
            device,
            queue,
            pipeline,
            uniforms_layout,
            image_layout,
            sampler,
            images: Vec::new(),
            banner: None,
            slots: Vec::new(),
            image_bytes: 0,
        }
    }

    fn add_image(&mut self, image: SpriteImage<'_>) -> Result<ImageId, RenderError> {
        check_image(image)?;
        self.check_size(image.width, image.height)?;
        let texture = upload_sprite(&self.device, &self.queue, image);
        Ok(self.store(texture, image.width, image.height))
    }

    fn add_canvas(&mut self, width: u32, height: u32) -> Result<ImageId, RenderError> {
        if width == 0 || height == 0 {
            return Err(RenderError::BadSprite { width, height, len: 0 });
        }
        self.check_size(width, height)?;
        // wgpu remplit de zéros une texture neuve : transparente
        let texture = new_texture(&self.device, width, height);
        Ok(self.store(texture, width, height))
    }

    fn paint_canvas(&mut self, id: ImageId, premultiplied: &[u8]) -> Result<(), RenderError> {
        let Some(Some(stored)) = self.images.get(id.0) else { return Err(RenderError::UnknownImage(id.0)) };
        let (width, height) = (stored.width, stored.height);
        check_image(SpriteImage { width, height, rgba: premultiplied })?;
        write_pixels(&self.queue, &stored.texture, width, height, premultiplied);
        // la copie part tout de suite, sans attendre le prochain dessin : une fenêtre occultée n'en
        // soumet aucun (`render` repart sur `Occluded`), et chaque copie de wgpu (8,3 Mo pour une
        // image de l'intro) resterait jusque-là. Une soumission, même vide, envoie les écritures en
        // attente puis rend ce que les soumissions finies tenaient (wgpu-core 30 : `Queue::submit`,
        // puis `Device::maintain`) : au plus une ou deux copies à la fois.
        self.queue.submit(std::iter::empty());
        Ok(())
    }

    fn remove_image(&mut self, id: ImageId) {
        if let Some(stored) = self.images.get_mut(id.0).and_then(Option::take) {
            self.image_bytes -= stored.bytes();
        }
    }

    fn settle(&self) {
        let wait = wgpu::PollType::Wait { submission_index: None, timeout: Some(SETTLE) };
        if let Err(err) = self.device.poll(wait) {
            log::debug!("carte graphique pas encore libre : {err}");
        }
    }

    /// La carte graphique accepte-t-elle une image de cette taille ?
    fn check_size(&self, width: u32, height: u32) -> Result<(), RenderError> {
        let max = self.device.limits().max_texture_dimension_2d;
        if width > max || height > max {
            return Err(RenderError::TooLarge { width, height, max });
        }
        Ok(())
    }

    /// Range une texture confiée, prête à être lue par le shader : son numéro.
    fn store(&mut self, texture: wgpu::Texture, width: u32, height: u32) -> ImageId {
        let bind_group = self.bind_texture(&texture);
        let stored = Stored { bind_group, texture, width, height };
        self.image_bytes += stored.bytes();
        self.images.push(Some(stored));
        ImageId(self.images.len() - 1)
    }

    fn set_banner(&mut self, image: Option<SpriteImage<'_>>) -> Result<(), RenderError> {
        self.banner = match image {
            Some(image) => {
                check_image(image)?;
                Some(self.bind_image(image))
            }
            None => None,
        };
        Ok(())
    }

    /// Envoie une image au GPU et la prépare à être lue par le shader.
    fn bind_image(&self, image: SpriteImage<'_>) -> wgpu::BindGroup {
        let texture = upload_sprite(&self.device, &self.queue, image);
        self.bind_texture(&texture)
    }

    /// Prépare une texture à être lue par le shader.
    fn bind_texture(&self, texture: &wgpu::Texture) -> wgpu::BindGroup {
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("capri image"),
            layout: &self.image_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) },
            ],
        })
    }

    /// Les commandes qui dessinent une image dans `view`, effacée d'abord.
    fn encode(
        &mut self,
        view: &wgpu::TextureView,
        viewport: [f32; 2],
        layers: &[(ImageId, SpriteDraw)],
        banner: Option<&SpriteDraw>,
    ) -> wgpu::CommandBuffer {
        // ce qui se dessine vraiment : une image inconnue est sautée, le bandeau seulement s'il existe
        let mut draws: Vec<(&wgpu::BindGroup, &SpriteDraw)> = Vec::with_capacity(layers.len() + 1);
        for (id, draw) in layers {
            match self.images.get(id.0) {
                Some(Some(image)) => draws.push((&image.bind_group, draw)),
                _ => log::warn!("image n° {} inconnue : pas dessinée", id.0),
            }
        }
        if let (Some(image), Some(draw)) = (self.banner.as_ref(), banner) {
            draws.push((image, draw));
        }
        // les réglages de chaque dessin, chacun dans le sien, créés au besoin
        while self.slots.len() < draws.len() {
            self.slots.push(Slot::new(&self.device, &self.uniforms_layout));
        }
        for ((_, draw), slot) in draws.iter().zip(&self.slots) {
            self.queue.write_buffer(&slot.buffer, 0, bytemuck::bytes_of(&Uniforms::of(draw, viewport)));
        }

        let mut encoder =
            self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("capri") });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("capri"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
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
            for ((image, _), slot) in draws.iter().zip(&self.slots) {
                pass.set_bind_group(0, &slot.bind_group, &[]);
                pass.set_bind_group(1, *image, &[]);
                pass.draw(0..6, 0..1);
            }
        }
        encoder.finish()
    }
}

impl Slot {
    fn new(device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> Self {
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("capri réglages"),
            contents: bytemuck::bytes_of(&Uniforms::zeroed()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("capri réglages"),
            layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: buffer.as_entire_binding() }],
        });
        Self { buffer, bind_group }
    }
}

/// Pour les essais : le même rendu, dans une image en mémoire plutôt que dans une fenêtre.
pub struct Offscreen {
    painter: Painter,
    texture: wgpu::Texture,
    width: u32,
    height: u32,
}

impl Offscreen {
    /// Une image de `width` × `height` pixels, transparente.
    pub fn new(width: u32, height: u32) -> Result<Self, RenderError> {
        let instance = wgpu::Instance::new(instance_descriptor());
        let (_, device, queue) = open(&instance, None)?;
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("capri hors écran"),
            size: wgpu::Extent3d { width: width.max(1), height: height.max(1), depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let painter = Painter::new(device, queue, format);
        Ok(Self { painter, texture, width: width.max(1), height: height.max(1) })
    }

    pub fn add_image(&mut self, image: SpriteImage<'_>) -> Result<ImageId, RenderError> {
        self.painter.add_image(image)
    }

    pub fn add_canvas(&mut self, width: u32, height: u32) -> Result<ImageId, RenderError> {
        self.painter.add_canvas(width, height)
    }

    pub fn paint_canvas(&mut self, id: ImageId, premultiplied: &[u8]) -> Result<(), RenderError> {
        self.painter.paint_canvas(id, premultiplied)
    }

    pub fn remove_image(&mut self, id: ImageId) {
        self.painter.remove_image(id);
    }

    pub fn settle(&self) {
        self.painter.settle();
    }

    pub fn set_banner(&mut self, image: Option<SpriteImage<'_>>) -> Result<(), RenderError> {
        self.painter.set_banner(image)
    }

    pub fn image_bytes(&self) -> u64 {
        self.painter.image_bytes
    }

    /// Dessine comme [`Renderer::render`], et rend l'image : RGBA 8 bits **prémultiplié**, ligne
    /// par ligne, sur fond transparent.
    pub fn render(
        &mut self,
        layers: &[(ImageId, SpriteDraw)],
        banner: Option<&SpriteDraw>,
    ) -> Result<Vec<u8>, RenderError> {
        let view = self.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let viewport = [self.width as f32, self.height as f32];
        let draw = self.painter.encode(&view, viewport, layers, banner);
        let row = (self.width * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let device = &self.painter.device;
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("capri relue"),
            size: u64::from(row) * u64::from(self.height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("capri relue") });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row),
                    rows_per_image: Some(self.height),
                },
            },
            wgpu::Extent3d { width: self.width, height: self.height, depth_or_array_layers: 1 },
        );
        self.painter.queue.submit([draw, encoder.finish()]);
        let (sender, receiver) = std::sync::mpsc::channel();
        buffer.map_async(wgpu::MapMode::Read, .., move |done| {
            let _ = sender.send(done);
        });
        device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|err| RenderError::Readback(err.to_string()))?;
        receiver
            .recv()
            .map_err(|err| RenderError::Readback(err.to_string()))?
            .map_err(|err| RenderError::Readback(err.to_string()))?;
        let mapped = buffer.get_mapped_range(..).map_err(|err| RenderError::Readback(err.to_string()))?;
        let mut pixels = Vec::with_capacity((self.width * self.height * 4) as usize);
        for y in 0..self.height as usize {
            let start = y * row as usize;
            pixels.extend_from_slice(&mapped[start..start + self.width as usize * 4]);
        }
        drop(mapped);
        buffer.unmap();
        Ok(pixels)
    }
}

/// La carte graphique : la puce intégrée de préférence, qui ménage la batterie.
fn open(
    instance: &wgpu::Instance,
    surface: Option<&wgpu::Surface<'_>>,
) -> Result<(wgpu::Adapter, wgpu::Device, wgpu::Queue), RenderError> {
    // Elle tourne toute la journée : la puce graphique intégrée suffit et ménage la batterie.
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        compatible_surface: surface,
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
    Ok((adapter, device, queue))
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

/// Envoie une image au GPU, prémultipliée (sinon ses bords auraient un liseré sombre).
fn upload_sprite(device: &wgpu::Device, queue: &wgpu::Queue, sprite: SpriteImage<'_>) -> wgpu::Texture {
    let texture = new_texture(device, sprite.width, sprite.height);
    write_pixels(queue, &texture, sprite.width, sprite.height, &premultiply(sprite.rgba));
    texture
}

/// Une texture RGBA 8 bits de cette taille, à lire par le shader et à remplir (de zéros d'abord).
fn new_texture(device: &wgpu::Device, width: u32, height: u32) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("capri sprite"),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

/// Réécrit toute la texture avec ces pixels (déjà prémultipliés), avant le prochain dessin : wgpu
/// les copie d'abord dans un tampon à lui, libéré une fois la copie faite par la carte graphique.
fn write_pixels(queue: &wgpu::Queue, texture: &wgpu::Texture, width: u32, height: u32, premultiplied: &[u8]) {
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        premultiplied,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4 * width),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
    );
}

fn premultiply(rgba: &[u8]) -> Vec<u8> {
    let mut out = rgba.to_vec();
    premultiply_in_place(&mut out);
    out
}

/// Prémultiplie des pixels RGBA 8 bits (comme dans un PNG) par leur alpha, sur place : ce que
/// [`Renderer::paint_canvas`] attend. Sans carte graphique : à faire hors du thread principal.
pub fn premultiply_in_place(rgba: &mut [u8]) {
    for px in rgba.as_chunks_mut::<4>().0 {
        let a = u16::from(px[3]);
        for c in &mut px[..3] {
            *c = ((u16::from(*c) * a + 127) / 255) as u8;
        }
    }
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
    fn premultiplying_in_place_gives_the_same_pixels() {
        let mut pixels = vec![200, 100, 50, 255, 200, 100, 50, 0, 255, 255, 255, 128];
        let copied = premultiply(&pixels);
        premultiply_in_place(&mut pixels);
        assert_eq!(pixels, copied);
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
        assert_eq!(std::mem::size_of::<Uniforms>(), 64);
    }

    #[test]
    fn a_crop_is_a_cell_of_its_grid_in_normalized_coordinates() {
        assert_eq!(Crop::pixels((0, 0), (10, 20), (10, 20)), Crop::WHOLE);
        // la case n° 16 d'une grille de 15 × 4 cases de 116 × 384 (1 740 × 1 536 px)
        let crop = Crop::pixels((116, 384), (116, 384), (1740, 1536));
        assert!((crop.min[0] - 1.0 / 15.0).abs() < 1e-6 && (crop.min[1] - 0.25).abs() < 1e-6);
        assert!((crop.size[0] - 1.0 / 15.0).abs() < 1e-6 && (crop.size[1] - 0.25).abs() < 1e-6);
        // une image vide ne divise pas par zéro
        assert!(Crop::pixels((0, 0), (1, 1), (0, 0)).size.iter().all(|v| v.is_finite()));
    }
}

#[cfg(test)]
mod gpu_tests {
    use super::*;

    /// Sur la vraie carte graphique : une case d'une grille se dessine seule, sans rien prendre à
    /// ses voisines, même agrandie (filtrage linéaire) ; la copie violette n'apparaît que si on la
    /// demande.
    #[test]
    #[ignore = "dessine sur la vraie carte graphique de cette machine"]
    fn a_cell_is_drawn_alone_without_bleeding_from_its_neighbours() {
        // quatre cases de 4 × 4 px côte à côte, chacune d'une couleur pleine
        let colors = [[255u8, 0, 0, 255], [0, 255, 0, 255], [0, 0, 255, 255], [255, 255, 0, 255]];
        let mut rgba = Vec::new();
        for _y in 0..4 {
            for color in colors {
                for _x in 0..4 {
                    rgba.extend_from_slice(&color);
                }
            }
        }
        let mut offscreen = Offscreen::new(64, 64).expect("une carte graphique");
        let grid = offscreen.add_image(SpriteImage { width: 16, height: 4, rgba: &rgba }).expect("confiée");
        assert_eq!(offscreen.image_bytes(), 16 * 4 * 4);
        let draw = SpriteDraw {
            min: [8.0, 8.0],
            size: [48.0, 48.0],
            ghost: [0.0, 0.0],
            ghost_strength: 0.0,
            boil_seed: 0,
            boil: 0.0,
            opacity: 1.0,
            crop: Crop::pixels((8, 0), (4, 4), (16, 4)),
        };
        let pixels = offscreen.render(&[(grid, draw)], None).expect("relue");
        let at = |x: usize, y: usize| &pixels[(y * 64 + x) * 4..(y * 64 + x) * 4 + 4];
        // partout dans le rectangle, et jusqu'à ses bords : le bleu de la case n° 2, rien d'autre
        for (x, y) in [(8, 8), (32, 32), (55, 55), (8, 55), (55, 8), (9, 30), (54, 30)] {
            assert_eq!(at(x, y), [0, 0, 255, 255], "({x}, {y})");
        }
        // dehors, transparent
        for (x, y) in [(7, 30), (56, 30), (30, 7), (30, 56), (0, 0)] {
            assert_eq!(at(x, y), [0, 0, 0, 0], "({x}, {y})");
        }
        // avec une copie violette demandée, elle apparaît à côté
        let ghosted = SpriteDraw { ghost: [4.0, 0.0], ghost_strength: GHOST, ..draw };
        let pixels = offscreen.render(&[(grid, ghosted)], None).expect("relue");
        let beside = &pixels[(30 * 64 + 58) * 4..(30 * 64 + 58) * 4 + 4];
        assert!(beside[3] > 0 && beside[2] > beside[1], "{beside:?}");
    }

    /// L'empreinte mémoire de ce processus (`phys_footprint`, mémoire graphique comprise sur un Mac
    /// à puce Apple), lue par `/usr/bin/footprint`.
    #[cfg(target_os = "macos")]
    fn footprint() -> u64 {
        let output = std::process::Command::new("/usr/bin/footprint")
            .args(["-p", &std::process::id().to_string(), "-f", "bytes"])
            .output()
            .expect("footprint");
        let text = String::from_utf8_lossy(&output.stdout).into_owned();
        text.split("Footprint: ")
            .nth(1)
            .and_then(|rest| rest.split(' ').next())
            .and_then(|n| n.parse().ok())
            .expect("son total")
    }

    /// Sur la vraie carte graphique : une toile peinte vingt fois sans que rien ne soit dessiné
    /// (une fenêtre occultée : `render` repart sans rien soumettre) ne garde pas vingt copies de
    /// transfert de wgpu, au plus quelques-unes.
    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "peint sur la vraie carte graphique de cette machine, et lit son empreinte mémoire"]
    fn painting_a_canvas_without_drawing_keeps_at_most_a_few_copies() {
        const IMAGE: u64 = 1920 * 1080 * 4;
        let mut offscreen = Offscreen::new(16, 16).expect("une carte graphique");
        let canvas = offscreen.add_canvas(1920, 1080).expect("une toile");
        let pixels = vec![200u8; IMAGE as usize];
        offscreen.paint_canvas(canvas, &pixels).expect("peinte");
        let _ = offscreen.render(&[], None).expect("rendue");
        let before = footprint();
        for _ in 0..20 {
            offscreen.paint_canvas(canvas, &pixels).expect("peinte");
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let grown = footprint().saturating_sub(before);
        println!("après 20 peintures sans dessin : +{:.1} Mo", grown as f64 / 1e6);
        assert!(grown < 4 * IMAGE, "+{:.1} Mo : les copies s'accumulent", grown as f64 / 1e6);
    }

    /// Sur la vraie carte graphique : une toile lâchée juste après une copie reste tenue tant que
    /// rien d'autre n'est soumis (une fenêtre cachée ne soumet plus rien), même longtemps ; après
    /// `settle`, la carte graphique la rend (Metal le fait un instant après : on attend 1 s au plus).
    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "peint sur la vraie carte graphique de cette machine, et lit son empreinte mémoire"]
    fn a_canvas_let_go_is_given_back_once_settled() {
        const IMAGE: u64 = 1920 * 1080 * 4;
        let mut offscreen = Offscreen::new(16, 16).expect("une carte graphique");
        let _ = offscreen.render(&[], None).expect("rendue");
        // ses pixels, gardés tout du long : seule la carte graphique change entre les mesures
        let pixels = vec![200u8; IMAGE as usize];
        let before = footprint();
        let canvas = offscreen.add_canvas(1920, 1080).expect("une toile");
        offscreen.paint_canvas(canvas, &pixels).expect("peinte");
        offscreen.remove_image(canvas);
        std::thread::sleep(std::time::Duration::from_millis(400));
        let held = footprint().saturating_sub(before);
        println!("lâchée, 400 ms après : +{:.1} Mo", held as f64 / 1e6);
        assert!(held > IMAGE / 2, "+{:.1} Mo : déjà rendue sans rien soumettre ?", held as f64 / 1e6);
        offscreen.settle();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
        let after = loop {
            let after = footprint().saturating_sub(before);
            if after < IMAGE / 4 || std::time::Instant::now() > deadline {
                break after;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        };
        println!("après settle : +{:.1} Mo", after as f64 / 1e6);
        assert!(after < IMAGE / 4, "+{:.1} Mo : toujours tenue", after as f64 / 1e6);
    }

    /// Sur la vraie carte graphique : une toile naît transparente, se réécrit en place (une seule
    /// texture, comptée une fois), et une fois lâchée, elle ne pèse plus rien et ne se dessine plus.
    #[test]
    #[ignore = "dessine sur la vraie carte graphique de cette machine"]
    fn a_canvas_is_repainted_in_place_and_released() {
        let mut offscreen = Offscreen::new(16, 16).expect("une carte graphique");
        let canvas = offscreen.add_canvas(4, 4).expect("une toile");
        assert_eq!(offscreen.image_bytes(), 4 * 4 * 4);
        let draw = SpriteDraw {
            min: [0.0, 0.0],
            size: [16.0, 16.0],
            ghost: [0.0, 0.0],
            ghost_strength: 0.0,
            boil_seed: 0,
            boil: 0.0,
            opacity: 1.0,
            crop: Crop::WHOLE,
        };
        let middle = |pixels: &[u8]| pixels[(8 * 16 + 8) * 4..(8 * 16 + 8) * 4 + 4].to_vec();
        assert_eq!(middle(&offscreen.render(&[(canvas, draw)], None).expect("relue")), [0, 0, 0, 0]);
        for color in [[255u8, 0, 0, 255], [0, 0, 255, 255], [0, 128, 0, 128]] {
            let pixels: Vec<u8> = color.iter().copied().cycle().take(4 * 4 * 4).collect();
            offscreen.paint_canvas(canvas, &pixels).expect("peinte");
            assert_eq!(middle(&offscreen.render(&[(canvas, draw)], None).expect("relue")), color);
        }
        assert_eq!(offscreen.image_bytes(), 4 * 4 * 4, "toujours une seule texture");
        assert!(matches!(offscreen.paint_canvas(canvas, &[0; 12]), Err(RenderError::BadSprite { .. })));
        offscreen.remove_image(canvas);
        assert_eq!(offscreen.image_bytes(), 0);
        offscreen.remove_image(canvas); // une seconde fois : rien
        assert!(matches!(offscreen.paint_canvas(canvas, &[0; 64]), Err(RenderError::UnknownImage(_))));
        assert_eq!(middle(&offscreen.render(&[(canvas, draw)], None).expect("relue")), [0, 0, 0, 0]);
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
