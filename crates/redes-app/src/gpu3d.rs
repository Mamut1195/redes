//! Pase 3D con búfer de profundidad (fase 5) e instanciado (F14-05).
//!
//! Rune es un framework 2D: dibuja instancias con el algoritmo del pintor. Este módulo añade un
//! pase propio de wgpu **dentro de la misma ventana**, sobre el mismo dispositivo y el mismo
//! target que Rune, con una textura de profundidad. Se dibuja:
//!
//! 1. el modelo con prueba de profundidad: prototipos indexados con instancias (piezas y
//!    modelos) y lotes fijos (tuberías, ciudad, terreno), con LOD y recorte por cámara
//!    ([`redes_core::scene3d`]);
//! 2. la interfaz de Rune encima, reabriendo el pase con `LoadOp::Load`.
//!
//! El canvas se pinta transparente para que el pase 3D se vea a través de él. Sin GPU o con
//! `--sin-gpu3d` la app usa el renderer 2D de siempre.

use std::sync::Arc;

use redes_core::scene3d::{self, DrawPlan, INSTANCE_BYTES, Scene, VERTEX_BYTES};
use rune::compose::{Compositor, Frame};
use rune::render::EffectSite;

/// The instance transform is the one in `scene3d::Instance::apply`. Normals go through the
/// inverse scale, so a stretched shaft is lit like the unstretched one.
const SHADER: &str = r#"
struct U {
    vp: mat4x4<f32>,
    light: vec4<f32>,
};
@group(0) @binding(0) var<uniform> u: U;

struct In {
    @location(0) pos: vec3<f32>,
    @location(1) nrm: vec3<f32>,
    @location(2) col: vec3<f32>,
    @location(3) offset: vec3<f32>,
    @location(4) scale: vec3<f32>,
    @location(5) rotation: vec2<f32>,
    @location(6) pitch: vec2<f32>,
};

struct Out {
    @builtin(position) clip: vec4<f32>,
    @location(0) col: vec3<f32>,
};

fn rotate_z(v: vec3<f32>, r: vec2<f32>) -> vec3<f32> {
    return vec3<f32>(v.x * r.x - v.y * r.y, v.x * r.y + v.y * r.x, v.z);
}

fn tilt_y(v: vec3<f32>, p: vec2<f32>) -> vec3<f32> {
    return vec3<f32>(v.x * p.x - v.z * p.y, v.y, v.x * p.y + v.z * p.x);
}

@vertex
fn vs(i: In) -> Out {
    var o: Out;
    let world = rotate_z(tilt_y(i.pos * i.scale, i.pitch), i.rotation) + i.offset;
    let normal = rotate_z(tilt_y(i.nrm / i.scale, i.pitch), i.rotation);
    o.clip = u.vp * vec4<f32>(world, 1.0);
    let l = abs(dot(normalize(normal), normalize(u.light.xyz)));
    o.col = i.col * (0.78 + 0.30 * l);
    return o;
}

@fragment
fn fs(i: Out) -> @location(0) vec4<f32> {
    return vec4<f32>(i.col, 1.0);
}
"#;

/// Lo que la app deja listo para el pase 3D.
///
/// La escena y la cámara van por separado (F14-02): mover la cámara solo cambia la matriz y el
/// plan (qué instancias, con qué LOD); la escena se regenera cuando cambia lo que la define
/// (proyecto, capas, biblioteca) y entonces sube `generation`. Los bytes se comparten con el
/// compositor sin copiarlos (F14-03).
#[derive(Clone, Debug)]
pub struct Geometry {
    pub scene: Arc<Scene>,
    pub plan: Arc<DrawPlan>,
    /// Matriz vista-proyección en columnas, con la exageración vertical incluida.
    pub view_proj: [f32; 16],
    pub px_per_metre: f32,
    /// Sube cada vez que se regenera la escena; el compositor solo la vuelve a subir entonces.
    pub generation: u64,
    /// Tope de bytes de GPU de la escena; el compositor lo baja al límite del dispositivo.
    pub budget: u64,
}

impl Default for Geometry {
    fn default() -> Self {
        Geometry {
            scene: Arc::default(),
            plan: Arc::default(),
            view_proj: [0.0; 16],
            px_per_metre: 1.0,
            generation: 0,
            budget: scene3d::GPU_BUDGET,
        }
    }
}

impl Geometry {
    #[cfg(test)]
    pub fn load(&mut self, p: &redes_core::model::Project, layers: redes_core::view::Layers) {
        self.load_with_models(p, layers, &[], &Default::default(), &mut Default::default());
    }

    /// Solo la cámara: matriz con la exageración vertical alrededor de la cota del objetivo y el
    /// plan de dibujo (recorte y LOD) para ella.
    pub fn set_camera(&mut self, cam: &redes_core::view::Camera) {
        let aspect = if cam.viewport[1] > 1.0 {
            cam.viewport[0] / cam.viewport[1]
        } else {
            1.6
        };
        self.view_proj = cam.matriz(aspect, cam.z_scale as f32);
        self.px_per_metre = scene3d::px_per_metre(cam);
        self.replan();
    }

    /// Regenera la escena (solo cuando algo la cambia), con los modelos de la biblioteca.
    pub fn load_with_models(
        &mut self,
        p: &redes_core::model::Project,
        layers: redes_core::view::Layers,
        models: &[crate::modelos::Instancia],
        pipes: &redes_core::pipe_layout::PipePieces,
        pieces: &mut redes_core::mallas::CachePiezas,
    ) {
        let placed: Vec<scene3d::PlacedModel> = models
            .iter()
            .map(|m| scene3d::PlacedModel {
                position: m.pos,
                mesh: Arc::clone(&m.malla),
                rotation_deg: m.giro,
                replaces: Some(m.nodo),
            })
            .collect();
        self.scene = Arc::new(Scene::build_with_pipes(
            p,
            layers,
            &placed,
            pipes,
            pieces,
            self.budget,
        ));
        // Nunca vuelve a 0: el compositor usa 0 para «búfer vacío, hay que subir».
        self.generation = self.generation.wrapping_add(1).max(1);
        self.replan();
    }

    fn replan(&mut self) {
        self.plan = Arc::new(scene3d::plan(
            &self.scene,
            &self.view_proj,
            self.px_per_metre,
        ));
    }

    /// Lo que el compositor toma en un frame: el plan siempre (un puntero); la escena solo si es
    /// de otra generación que la ya subida.
    pub fn take(&self, uploaded: u64) -> Snapshot {
        Snapshot {
            upload: (self.generation != uploaded).then(|| Arc::clone(&self.scene)),
            plan: Arc::clone(&self.plan),
            view_proj: self.view_proj,
            generation: self.generation,
        }
    }
}

/// Lectura de un frame: con el mutex tomado solo se copian la matriz y dos punteros.
pub struct Snapshot {
    /// Escena que hay que subir a la GPU, o `None` si los búferes ya tienen esta generación.
    pub upload: Option<Arc<Scene>>,
    pub plan: Arc<DrawPlan>,
    pub view_proj: [f32; 16],
    pub generation: u64,
}

struct SceneBuffers {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    instances: wgpu::Buffer,
    instance_capacity: u64,
    /// Plan whose instances are in `instances`.
    plan: Option<Arc<DrawPlan>>,
}

struct Resources {
    pipeline: wgpu::RenderPipeline,
    bind: wgpu::BindGroup,
    depth: wgpu::Texture,
    depth_view: wgpu::TextureView,
    size: (u32, u32),
    format: wgpu::TextureFormat,
    uniform: wgpu::Buffer,
    scene: Option<SceneBuffers>,
}

/// Compositor que dibuja el modelo en 3D y luego la interfaz encima.
pub struct Compositor3d {
    uploaded_generation: u64,
    resources: Option<Resources>,
}

/// Tope de la escena para un dispositivo cuyo `max_buffer_size` es `limit`.
///
/// La escena entera (vértices + índices + instancias) no pasa de `scene3d::GPU_BUDGET`, así que
/// tampoco pasa ninguno de sus búferes. Antes el tope era 512 MB fijo y una GPU con 256 MB
/// entraba en pánico al crear el búfer con ~5k pozos.
pub fn device_budget(limit: u64) -> u64 {
    let budget = limit.min(scene3d::GPU_BUDGET);
    (budget / 4 * 4).max(4)
}

fn vertex_layouts() -> [wgpu::VertexBufferLayout<'static>; 2] {
    const VERTEX: [wgpu::VertexAttribute; 3] =
        wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x3];
    const INSTANCE: [wgpu::VertexAttribute; 4] = wgpu::vertex_attr_array![
        3 => Float32x3,
        4 => Float32x3,
        5 => Float32x2,
        6 => Float32x2
    ];
    [
        wgpu::VertexBufferLayout {
            array_stride: VERTEX_BYTES as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &VERTEX,
        },
        wgpu::VertexBufferLayout {
            array_stride: INSTANCE_BYTES as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &INSTANCE,
        },
    ]
}

impl Compositor3d {
    pub fn new() -> Compositor3d {
        Compositor3d {
            uploaded_generation: 0,
            resources: None,
        }
    }

    fn ensure(&mut self, device: &wgpu::Device, size: (u32, u32), format: wgpu::TextureFormat) {
        let stale = match &self.resources {
            None => true,
            Some(r) => r.size != size || r.format != format,
        };
        if !stale {
            return;
        }
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("redes-3d"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("redes-3d-bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("redes-3d-layout"),
            bind_group_layouts: &[Some(&bind_layout)],
            immediate_size: 0,
        });
        let buffers = vertex_layouts();
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("redes-3d-pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[Some(buffers[0].clone()), Some(buffers[1].clone())],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("redes-3d-uniforme"),
            size: 80,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("redes-3d-bind"),
            layout: &bind_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });
        let depth = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("redes-3d-depth"),
            size: wgpu::Extent3d {
                width: size.0.max(1),
                height: size.1.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let depth_view = depth.create_view(&Default::default());
        self.resources = Some(Resources {
            pipeline,
            bind,
            depth,
            depth_view,
            size,
            format,
            uniform,
            scene: None,
        });
        // Recursos nuevos = sin búferes de escena: hay que volver a subirla.
        self.uploaded_generation = 0;
    }

    fn upload_scene(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, scene: &Scene) {
        let Some(r) = self.resources.as_mut() else {
            return;
        };
        let buffer = |label: &str, size: u64, usage: wgpu::BufferUsages| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: size.max(4),
                usage: usage | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        let vertices = buffer(
            "redes-3d-vertices",
            scene.vertices.len() as u64,
            wgpu::BufferUsages::VERTEX,
        );
        let indices = buffer(
            "redes-3d-indices",
            scene.indices.len() as u64,
            wgpu::BufferUsages::INDEX,
        );
        let instance_capacity = scene.instance_buffer_bytes() as u64;
        let instances = buffer(
            "redes-3d-instancias",
            instance_capacity,
            wgpu::BufferUsages::VERTEX,
        );
        queue.write_buffer(&vertices, 0, &scene.vertices);
        queue.write_buffer(&indices, 0, &scene.indices);
        r.scene = Some(SceneBuffers {
            vertices,
            indices,
            instances,
            instance_capacity,
            plan: None,
        });
    }

    fn upload_plan(&mut self, queue: &wgpu::Queue, plan: &Arc<DrawPlan>) {
        let Some(b) = self.resources.as_mut().and_then(|r| r.scene.as_mut()) else {
            return;
        };
        if b.plan.as_ref().is_some_and(|p| Arc::ptr_eq(p, plan)) {
            return;
        }
        let n = plan.instances.len().min(b.instance_capacity as usize);
        queue.write_buffer(&b.instances, 0, &plan.instances[..n]);
        b.plan = Some(Arc::clone(plan));
    }

    /// The device limit is only known here: a lower one rebuilds the scene with that budget.
    fn apply_device_budget(f: &mut Frame<'_, crate::State>) {
        let budget = device_budget(f.device.limits().max_buffer_size);
        let lower =
            f.rt.state
                .geom
                .lock()
                .map(|mut g| std::mem::replace(&mut g.budget, budget) != budget)
                .unwrap_or(false);
        if lower {
            f.rt.state.clave_malla = None;
            f.rt.state.rebuild();
        }
    }
}

impl Compositor<crate::State> for Compositor3d {
    fn render(&mut self, f: &mut Frame<'_, crate::State>) {
        let (device, queue, size, format) = (f.device, f.queue, f.size, f.format);
        Self::apply_device_budget(f);
        // La geometría la mantiene el estado de la app; el compositor solo la consume: el `Arc`
        // de la escena viene solo si hay una generación nueva que subir (F14-03).
        let geom = f.rt.state.geom.clone();
        let Ok(snap) = geom.lock().map(|g| g.take(self.uploaded_generation)) else {
            f.draw_ui();
            return;
        };
        if snap.generation == 0 {
            f.draw_ui();
            return;
        }
        self.ensure(device, size, format);
        if let Some(scene) = &snap.upload {
            self.upload_scene(device, queue, scene);
            self.uploaded_generation = snap.generation;
        }
        self.upload_plan(queue, &snap.plan);
        let Some(r) = &self.resources else {
            f.draw_ui();
            return;
        };

        // El pase se limita al rectángulo del lienzo (en píxeles de dispositivo), no a la ventana:
        // si no, el modelo se estira y se dibuja debajo de los paneles.
        let escala = f.rt.env.scale.max(0.1);
        let rect = f.rt.state.canvas_rect;
        let (vx, vy, vw, vh) = (
            (rect[0] * escala).round().max(0.0) as u32,
            (rect[1] * escala).round().max(0.0) as u32,
            ((rect[2] * escala).round()).max(1.0) as u32,
            ((rect[3] * escala).round()).max(1.0) as u32,
        );
        let (vx, vy) = (
            vx.min(size.0.saturating_sub(1)),
            vy.min(size.1.saturating_sub(1)),
        );
        let vw = vw.min(size.0.saturating_sub(vx)).max(1);
        let vh = vh.min(size.1.saturating_sub(vy)).max(1);
        let mut uniform = [0f32; 20];
        uniform[..16].copy_from_slice(&snap.view_proj);
        uniform[16..19].copy_from_slice(&[0.45, 0.52, 0.72]);
        uniform[19] = 1.0;
        queue.write_buffer(&r.uniform, 0, bytemap(&uniform));

        let mut pass = f.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("redes-3d"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: f.target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.050,
                        g: 0.055,
                        b: 0.066,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &r.depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_viewport(vx as f32, vy as f32, vw as f32, vh as f32, 0.0, 1.0);
        pass.set_scissor_rect(vx, vy, vw, vh);
        if let Some(b) = &r.scene {
            pass.set_pipeline(&r.pipeline);
            pass.set_bind_group(0, &r.bind, &[]);
            pass.set_vertex_buffer(0, b.vertices.slice(..));
            pass.set_index_buffer(b.indices.slice(..), wgpu::IndexFormat::Uint32);
            draw_plan(&mut pass, b, &snap.plan);
        }
        drop(pass);

        // La interfaz se dibuja encima, reabriendo el pase sin limpiar el color.
        let count = f.rt.instances.len() as u32;
        f.ui.prepare(device, queue, &mut f.rt.text.atlas, &f.rt.instances, size);
        let mut hook = |_p: &mut wgpu::RenderPass<'_>, _s: &EffectSite<'_>| {};
        f.ui.encode_layers_with(f.encoder, &mut hook);
        let mut pase_ui = f.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("redes-ui"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: f.target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        f.ui.draw_range_with(&mut pase_ui, format, 0..count, &mut hook);
    }

    fn draws_own(&self) -> bool {
        true
    }
}

/// One draw per prototype or batch. The instance buffer is bound at each draw's first instance
/// instead of passing a base instance, which not every backend supports.
fn draw_plan(pass: &mut wgpu::RenderPass<'_>, b: &SceneBuffers, plan: &DrawPlan) {
    let stride = INSTANCE_BYTES as u64;
    for d in &plan.draws {
        let start = d.first_instance as u64 * stride;
        let end = start + d.instance_count as u64 * stride;
        if d.instance_count == 0 || end > b.instance_capacity {
            continue;
        }
        pass.set_vertex_buffer(1, b.instances.slice(start..end));
        let indices = d.mesh.first_index..d.mesh.first_index + d.mesh.index_count;
        pass.draw_indexed(indices, d.mesh.base_vertex, 0..d.instance_count);
    }
}

fn bytemap(v: &[f32]) -> &[u8] {
    // Seguro: f32 tiene alineación 4 y el buffer es un uniforme de floats.
    unsafe { std::slice::from_raw_parts(v.as_ptr() as *const u8, std::mem::size_of_val(v)) }
}

/// ¿La app pidió el pase 3D? (`--gpu3d` o `REDES_GPU3D=1`)
pub fn activo() -> bool {
    let args = rune::extra_args();
    let por_flag = args.iter().any(|a| a == "--gpu3d");
    let por_entorno = std::env::var("REDES_GPU3D")
        .map(|v| v == "1")
        .unwrap_or(false);
    por_flag || por_entorno
}

#[cfg(test)]
mod tests {
    use super::{Geometry, device_budget};

    const MB: u64 = 1024 * 1024;
    use redes_core::scene3d::{GPU_BUDGET, Scene};
    use redes_core::view::Layers;

    /// The budget comes from the device (P0): with `max_buffer_size` = 256 MB no buffer goes
    /// over 256 MB, and a scene over that budget is trimmed with a warning. The budget used to be
    /// a fixed 512 MB and `create_buffer` panicked.
    #[test]
    fn the_budget_respects_the_device_limit() {
        let limit = 256 * MB;
        let budget = device_budget(limit);
        assert!(budget <= limit, "budget {budget} > limit {limit}");
        assert_eq!(budget % 4, 0, "write_buffer needs multiples of 4");
        assert!(device_budget(MB) <= MB);
        assert_eq!(device_budget(u64::MAX), GPU_BUDGET);

        let p = redes_core::demo_city::ciudad();
        let mut pieces = Default::default();
        let whole = Scene::build(&p, Layers::default(), &[], &mut pieces, GPU_BUDGET);
        let small = device_budget(whole.requested_bytes / 2);
        let trimmed = Scene::build(&p, Layers::default(), &[], &mut pieces, small);
        assert!(trimmed.gpu_bytes() <= small);
        assert!(trimmed.warning.is_some());
    }

    /// Apagar una capa reduce lo que se dibuja, y lo que el plan dibuja está en los búferes.
    #[test]
    fn layers_reduce_what_is_drawn() {
        let p = redes_core::demo_city::ciudad();
        let mut cam = redes_core::view::Camera {
            viewport: [900.0, 600.0],
            ..Default::default()
        };
        cam.fit(&p);
        let mut all = Geometry::default();
        all.load(&p, Layers::default());
        all.set_camera(&cam);
        assert!(all.plan.triangles > 0);
        let no_sewer = Layers {
            alcantarillado: false,
            ..Default::default()
        };
        let mut part = Geometry::default();
        part.load(&p, no_sewer);
        part.set_camera(&cam);
        assert!(
            part.plan.triangles < all.plan.triangles,
            "{} vs {}",
            part.plan.triangles,
            all.plan.triangles
        );
        for d in &all.plan.draws {
            let end = (d.mesh.first_index + d.mesh.index_count) as usize * 4;
            assert!(end <= all.scene.indices.len());
        }
    }

    /// Rebuilding without changes in the pieces generates none (F14-06).
    #[test]
    fn the_second_rebuild_generates_no_pieces() {
        let p = redes_core::demo_city::ciudad();
        let mut pieces = redes_core::mallas::CachePiezas::default();
        let mut g = Geometry::default();
        g.load_with_models(&p, Layers::default(), &[], &Default::default(), &mut pieces);
        let first = pieces.generadas();
        assert!(first > 0, "the first rebuild generates the pieces");
        let layers = Layers {
            edificios: false,
            ..Default::default()
        };
        g.load_with_models(&p, layers, &[], &Default::default(), &mut pieces);
        g.load_with_models(&p, Layers::default(), &[], &Default::default(), &mut pieces);
        assert_eq!(pieces.generadas(), first, "0 new meshes");
        assert!(!g.scene.is_empty());
    }

    /// With the scene still the compositor uploads nothing (F14-03): the first take brings the
    /// scene (the same `Arc`, no copy) and later ones of the same generation bring nothing even
    /// if the camera moves; the plan follows the camera. A new scene is uploaded again.
    #[test]
    fn a_still_scene_is_not_uploaded_again() {
        let p = redes_core::demo::project();
        let mut cam = redes_core::view::Camera {
            viewport: [900.0, 600.0],
            ..Default::default()
        };
        cam.fit(&p);
        let mut g = Geometry::default();
        g.load(&p, Layers::default());
        g.set_camera(&cam);

        let first = g.take(0);
        let scene = first.upload.expect("the first take uploads the scene");
        assert!(
            std::sync::Arc::ptr_eq(&scene, &g.scene),
            "shared, not copied"
        );
        let uploaded = first.generation;
        assert_ne!(uploaded, 0, "a loaded scene is never the empty generation");

        for _ in 0..3 {
            assert!(g.take(uploaded).upload.is_none(), "still frame: 0 copies");
        }
        cam.orbit(30.0, 5.0);
        g.set_camera(&cam);
        let moved = g.take(uploaded);
        assert!(moved.upload.is_none(), "moving the camera does not upload");
        assert_ne!(moved.view_proj, first.view_proj, "but the matrix changes");
        assert!(!std::sync::Arc::ptr_eq(&moved.plan, &first.plan));

        g.load(&p, Layers::default());
        assert!(g.take(uploaded).upload.is_some(), "a new scene is uploaded");
    }

    /// Vertical exaggeration lives in the matrix, not in the vertices: the same point with the
    /// `z_scale` k matrix lands where the exaggerated vertex landed with the scale-1 matrix.
    #[test]
    fn vertical_exaggeration_lives_in_the_matrix() {
        let cam = redes_core::view::Camera {
            viewport: [900.0, 600.0],
            target: [10.0, -4.0, 2500.0],
            distance: 80.0,
            z_scale: 2.6,
            ..Default::default()
        };
        let mut g = Geometry::default();
        g.set_camera(&cam);
        let m_k = g.view_proj;
        let m_1 = cam.matriz(1.5, 1.0);
        let m_k_ok = cam.matriz(1.5, 2.6);
        assert_eq!(m_k, m_k_ok, "set_camera uses the camera aspect and z_scale");
        let apply = |m: &[f32; 16], p: [f64; 3]| -> [f64; 3] {
            let c = |i: usize| -> f64 {
                (0..3).map(|j| m[j * 4 + i] as f64 * p[j]).sum::<f64>() + m[12 + i] as f64
            };
            let w = c(3);
            [c(0) / w, c(1) / w, c(2) / w]
        };
        let point = [3.0, 7.0, 2496.0];
        let exaggerated = [3.0, 7.0, (2496.0 - 2500.0) * 2.6 + 2500.0];
        let a = apply(&m_k_ok, point);
        let b = apply(&m_1, exaggerated);
        for i in 0..3 {
            assert!((a[i] - b[i]).abs() < 1e-4, "axis {i}: {} vs {}", a[i], b[i]);
        }
    }
}
