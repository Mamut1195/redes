//! Pase 3D con búfer de profundidad (fase 5).
//!
//! Rune es un framework 2D: dibuja instancias con el algoritmo del pintor. Este módulo añade un
//! pase propio de wgpu **dentro de la misma ventana**, sobre el mismo dispositivo y el mismo
//! target que Rune, con una textura de profundidad. Se dibuja:
//!
//! 1. el modelo (triángulos con normal y color) con prueba de profundidad;
//! 2. la interfaz de Rune encima, reabriendo el pase con `LoadOp::Load`.
//!
//! El canvas se pinta transparente para que el pase 3D se vea a través de él. Sin GPU o con
//! `--sin-gpu3d` la app usa el renderer 2D de siempre.

use rune::compose::{Compositor, Frame};
use rune::render::EffectSite;

const SHADER: &str = r#"
struct U {
    vp: mat4x4<f32>,
    luz: vec4<f32>,
};
@group(0) @binding(0) var<uniform> u: U;

struct In {
    @location(0) pos: vec3<f32>,
    @location(1) nrm: vec3<f32>,
    @location(2) col: vec3<f32>,
};

struct Out {
    @builtin(position) clip: vec4<f32>,
    @location(0) col: vec3<f32>,
};

@vertex
fn vs(i: In) -> Out {
    var o: Out;
    o.clip = u.vp * vec4<f32>(i.pos, 1.0);
    let l = abs(dot(normalize(i.nrm), normalize(u.luz.xyz)));
    o.col = i.col * (0.78 + 0.30 * l);
    return o;
}

@fragment
fn fs(i: Out) -> @location(0) vec4<f32> {
    return vec4<f32>(i.col, 1.0);
}
"#;

/// Geometría que la app deja lista para el pase 3D.
#[derive(Clone, Debug, Default)]
pub struct Geometria {
    /// Vértices empaquetados: posición (3f), normal (3f), color (3f) = 36 bytes.
    pub bytes: Vec<u8>,
    pub vertices: u32,
    /// Matriz vista-proyección en columnas (16 floats).
    pub vp: [f32; 16],
    pub generacion: u64,
}

impl Geometria {
    /// Recalcula la geometría a partir del proyecto (solo cuando algo cambia).
    pub fn cargar(
        &mut self,
        p: &redes_core::model::Project,
        cam: &redes_core::view::Camera,
        capas: redes_core::view::Layers,
    ) {
        self.cargar_con_modelos(p, cam, capas, &[]);
    }

    /// Igual que [`Self::cargar`] pero añade las mallas de la biblioteca ya colocadas.
    pub fn cargar_con_modelos(
        &mut self,
        p: &redes_core::model::Project,
        cam: &redes_core::view::Camera,
        capas: redes_core::view::Layers,
        instancias: &[crate::modelos::Instancia],
    ) {
        let malla = redes_core::gltf::triangulos_exportables(p, capas.terreno);
        // Se respetan las capas de la interfaz: apagar «Edificios» también los apaga aquí.
        let visibles: Vec<u16> = malla
            .familias
            .iter()
            .enumerate()
            .filter(|(_, nombre)| {
                let es_agua = nombre.starts_with("agua.");
                let es_alc = nombre.starts_with("saneamiento.");
                if es_agua && !capas.agua {
                    return false;
                }
                if es_alc && !capas.alcantarillado {
                    return false;
                }
                if !es_agua && !es_alc && nombre.as_str() != "terreno" && !capas.edificios {
                    return false;
                }
                true
            })
            .map(|(i, _)| i as u16)
            .collect();
        let k = cam.z_scale as f32;
        let zr = cam.target[2] as f32;
        let _ = k;
        let aspecto = if cam.viewport[1] > 1.0 {
            cam.viewport[0] / cam.viewport[1]
        } else {
            1.6
        };
        // La z ya viene exagerada en los vértices: la matriz va en escala 1.
        self.vp = cam.matriz(aspecto, 1.0);
        self.bytes.clear();
        self.bytes.reserve(malla.tris.len() * 3 * 36);
        // La app dibuja con z exagerada: se aplica aquí, una sola vez por reconstrucción.
        let mut contador: u32 = 0;
        let mut empujar = |pos: [f32; 3], normal: [f32; 3], color: [f32; 3]| {
            let z = (pos[2] - zr) * k + zr;
            for v in [
                pos[0], pos[1], z, normal[0], normal[1], normal[2], color[0], color[1], color[2],
            ] {
                self.bytes.extend_from_slice(&v.to_le_bytes());
            }
        };
        for t in &malla.tris {
            if !visibles.contains(&t.familia) {
                continue;
            }
            for k in 0..3 {
                empujar(t.pos[k], t.normal, t.color);
                contador += 1;
            }
        }
        // Modelos de la biblioteca: la malla ya viene en Z-arriba; se coloca en la cota del objeto.
        for inst in instancias {
            let (sen, cos) = (inst.giro.to_radians().sin(), inst.giro.to_radians().cos());
            for v in inst.malla.iter() {
                let (x, y) = (v.pos[0], v.pos[1]);
                let pos = [
                    inst.pos[0] + x * cos - y * sen,
                    inst.pos[1] + x * sen + y * cos,
                    inst.pos[2] + v.pos[2],
                ];
                empujar(pos, v.normal, v.color);
                contador += 1;
            }
        }
        // Lo que se declara es lo que se escribió: antes se contaba la malla entera aunque las
        // capas filtraran triángulos, y el pase leía vértices del frame anterior.
        self.vertices = contador;
        self.generacion = self.generacion.wrapping_add(1);
    }
}

struct Recursos {
    pipeline: wgpu::RenderPipeline,
    bind: wgpu::BindGroup,
    depth: wgpu::Texture,
    depth_view: wgpu::TextureView,
    tamano: (u32, u32),
    formato: wgpu::TextureFormat,
    vertices: wgpu::Buffer,
    uniforme: wgpu::Buffer,
    capacidad: u64,
}

/// Compositor que dibuja el modelo en 3D y luego la interfaz encima.
pub struct Compositor3d {
    ultima_generacion: u64,
    recursos: Option<Recursos>,
}

impl Compositor3d {
    pub fn new() -> Compositor3d {
        Compositor3d {
            ultima_generacion: 0,
            recursos: None,
        }
    }

    fn asegurar(&mut self, device: &wgpu::Device, size: (u32, u32), formato: wgpu::TextureFormat) {
        let rehacer = match &self.recursos {
            None => true,
            Some(r) => r.tamano != size || r.formato != formato,
        };
        if !rehacer {
            return;
        }
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("redes-3d"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let modulo = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
            bind_group_layouts: &[Some(&modulo)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("redes-3d-pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: 36,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: 0,
                            shader_location: 0,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: 12,
                            shader_location: 1,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: 24,
                            shader_location: 2,
                        },
                    ],
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: formato,
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
        let uniforme = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("redes-3d-uniforme"),
            size: 80,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("redes-3d-bind"),
            layout: &modulo,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniforme.as_entire_binding(),
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
        let capacidad = 36 * 3 * 200_000;
        let vertices = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("redes-3d-vertices"),
            size: capacidad,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.recursos = Some(Recursos {
            pipeline,
            bind,
            depth,
            depth_view,
            tamano: size,
            formato,
            vertices,
            uniforme,
            capacidad,
        });
        // Recursos nuevos = búfer vacío: hay que volver a subir la geometría.
        self.ultima_generacion = 0;
    }

    /// Crecen el búfer si la geometría no cabe (hasta un tope prudente).
    fn crecer(&mut self, device: &wgpu::Device, bytes: usize) {
        const TOPE: u64 = 512 * 1024 * 1024;
        let Some(r) = self.recursos.as_mut() else {
            return;
        };
        if bytes as u64 <= r.capacidad {
            return;
        }
        let paso = 4 * 1024 * 1024;
        let nueva = (((bytes as u64 + paso - 1) / paso) * paso).min(TOPE);
        r.vertices = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("redes-3d-vertices"),
            size: nueva,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        r.capacidad = nueva;
        self.ultima_generacion = 0;
        if bytes as u64 > TOPE {
            eprintln!(
                "redes: la escena tiene {} MB de vértices y el tope es {} MB; se recorta",
                bytes / (1024 * 1024),
                TOPE / (1024 * 1024)
            );
        }
    }
}

impl Compositor<crate::State> for Compositor3d {
    fn render(&mut self, f: &mut Frame<'_, crate::State>) {
        let (device, queue, size, formato) = (f.device, f.queue, f.size, f.format);
        // La geometría la mantiene el estado de la app; el compositor solo la consume.
        let geom = f.rt.state.geom.clone();
        let (bytes, vertices, vp, generacion) = {
            let g = geom.lock().unwrap();
            (g.bytes.clone(), g.vertices, g.vp, g.generacion)
        };
        if vertices == 0 {
            f.draw_ui();
            return;
        }
        self.asegurar(device, size, formato);
        self.crecer(device, bytes.len());
        let Some(r) = &self.recursos else {
            f.draw_ui();
            return;
        };

        if generacion != self.ultima_generacion {
            let n = bytes.len().min(r.capacidad as usize) as u64;
            queue.write_buffer(&r.vertices, 0, &bytes[..n as usize]);
            self.ultima_generacion = generacion;
        }

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
        let mut uniforme = [0f32; 20];
        uniforme[..16].copy_from_slice(&vp);
        uniforme[16..19].copy_from_slice(&[0.45, 0.52, 0.72]);
        uniforme[19] = 1.0;
        queue.write_buffer(&r.uniforme, 0, bytemap(&uniforme));

        let mut pase = f.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
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
        pase.set_viewport(vx as f32, vy as f32, vw as f32, vh as f32, 0.0, 1.0);
        pase.set_scissor_rect(vx, vy, vw, vh);
        pase.set_pipeline(&r.pipeline);
        pase.set_bind_group(0, &r.bind, &[]);
        pase.set_vertex_buffer(0, r.vertices.slice(..));
        let dibujables = vertices.min((r.capacidad / 36) as u32);
        pase.draw(0..dibujables, 0..1);
        drop(pase);

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
        f.ui.draw_range_with(&mut pase_ui, formato, 0..count, &mut hook);
    }

    fn draws_own(&self) -> bool {
        true
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
    use super::Geometria;
    use redes_core::view::Layers;

    /// Lo que el pase declara dibujar es exactamente lo que hay en el búfer, y apagar una capa
    /// reduce el conteo (antes se declaraba la malla entera y se leía basura del frame anterior).
    #[test]
    fn los_vertices_declarados_son_los_escritos() {
        let p = redes_core::demo_city::ciudad();
        let mut cam = redes_core::view::Camera::default();
        cam.viewport = [900.0, 600.0];
        cam.fit(&p);

        let mut todo = Geometria::default();
        todo.cargar(&p, &cam, Layers::default());
        assert!(todo.vertices > 0);
        assert_eq!(
            todo.bytes.len() as u32,
            todo.vertices * 36,
            "vértices declarados y bytes escritos deben coincidir"
        );

        let mut sin_alcantarillado = Layers::default();
        sin_alcantarillado.alcantarillado = false;
        let mut parte = Geometria::default();
        parte.cargar(&p, &cam, sin_alcantarillado);
        assert!(
            parte.vertices < todo.vertices,
            "apagar alcantarillado debe reducir los vértices ({} vs {})",
            parte.vertices,
            todo.vertices
        );
        assert_eq!(parte.bytes.len() as u32, parte.vertices * 36);
    }
}
