#[cfg(feature = "winit")]
pub mod window;

/// Order of components is: A, R, G, B
#[derive(Clone, Copy, Debug, Hash, PartialEq, PartialOrd)]
pub struct Color(pub u32);

impl Color {
    pub const BLACK_TRANSPARENT: Self = Self(0x0);
    pub const BLACK_OPAQUE: Self = Self(0xFF000000);
    pub const RED: Self = Self(0xFF0000FF);
    pub const GREEN: Self = Self(0xFF00FF00);
    pub const BLUE: Self = Self(0xFFFF0000);

    fn import(value: f32) -> u32 {
        (value.clamp(0.0, 1.0) * 255.0) as u32
    }

    pub fn new(red: f32, green: f32, blue: f32, alpha: f32) -> Self {
        Self(
            (Self::import(alpha) << 24)
                | (Self::import(red) << 16)
                | (Self::import(green) << 8)
                | Self::import(blue),
        )
    }

    fn export(self, index: u32) -> f32 {
        ((self.0 >> (index << 3)) & 0xFF) as f32 / 255.0
    }
    pub fn red(self) -> f32 {
        self.export(2)
    }
    pub fn green(self) -> f32 {
        self.export(1)
    }
    pub fn blue(self) -> f32 {
        self.export(0)
    }
    pub fn alpha(self) -> f32 {
        self.export(3)
    }
}

impl From<Color> for wgpu::Color {
    fn from(c: Color) -> Self {
        Self {
            r: c.red() as f64,
            g: c.green() as f64,
            b: c.blue() as f64,
            a: c.alpha() as f64,
        }
    }
}

impl Default for Color {
    fn default() -> Self {
        Color::BLACK_OPAQUE
    }
}

#[cfg_attr(not(feature = "winit"), allow(dead_code))]
struct SurfaceContext {
    raw: wgpu::Surface,
    format: wgpu::TextureFormat,
    size: wgpu::Extent3d,
}

pub struct Context {
    _instance: wgpu::Instance,
    surface: Option<SurfaceContext>,
    device: wgpu::Device,
    queue: wgpu::Queue,
}

#[derive(Default)]
pub struct ContextBuilder<'a> {
    #[cfg(feature = "winit")]
    window: Option<&'a window::Window>,
    #[cfg(not(feature = "winit"))]
    _window: Option<&'a ()>,
    power_preference: wgpu::PowerPreference,
}

impl<'a> ContextBuilder<'a> {
    #[cfg(feature = "winit")]
    pub fn screen(self, win: &'a window::Window) -> Self {
        Self {
            window: Some(win),
            ..self
        }
    }

    pub fn power_hungry(self, hungry: bool) -> Self {
        Self {
            power_preference: if hungry {
                wgpu::PowerPreference::HighPerformance
            } else {
                wgpu::PowerPreference::LowPower
            },
            ..self
        }
    }

    pub async fn build(self) -> Context {
        let instance = wgpu::Instance::new(wgpu::Backends::PRIMARY);
        #[cfg_attr(not(feature = "winit"), allow(unused_mut))]
        let mut surface = None;

        #[cfg(feature = "winit")]
        if let Some(win) = self.window {
            let size = win.raw.inner_size();
            let raw = unsafe { instance.create_surface(&win.raw) };
            surface = Some(SurfaceContext {
                raw,
                format: wgpu::TextureFormat::Rgba8Unorm,
                size: wgpu::Extent3d {
                    width: size.width,
                    height: size.height,
                    depth_or_array_layers: 1,
                },
            });
        }

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: self.power_preference,
                #[cfg(feature = "winit")]
                compatible_surface: surface.as_ref().map(|sc| &sc.raw),
                #[cfg(not(feature = "winit"))]
                compatible_surface: None,
            })
            .await
            .unwrap();
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default(), None)
            .await
            .unwrap();

        #[cfg(feature = "winit")]
        if let Some(ref mut suf) = surface {
            suf.format = suf.raw.get_preferred_format(&adapter).unwrap();
            suf.raw.configure(
                &device,
                &wgpu::SurfaceConfiguration {
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    format: suf.format,
                    width: suf.size.width,
                    height: suf.size.height,
                    present_mode: wgpu::PresentMode::Mailbox,
                },
            );
        }

        Context {
            _instance: instance,
            surface,
            device,
            queue,
        }
    }
}

impl Context {
    pub fn new<'a>() -> ContextBuilder<'a> {
        ContextBuilder::default()
    }

    pub fn render_screen(&mut self, scene: &Scene) {
        let surface = self.surface.as_mut().expect("No scren is configured!");
        let frame = surface.raw.get_current_frame().unwrap();
        let view = frame
            .output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut comb = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        {
            let _pass = comb.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("screen"),
                color_attachments: &[wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(scene.background.into()),
                        store: true,
                    },
                }],
                depth_stencil_attachment: None,
            });
        }

        self.queue.submit(vec![comb.finish()]);
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        // Do we need explicit cleanup?
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct NodeRef(u32);

pub type EntityRef = hecs::Entity;

#[derive(Debug, PartialEq)]
struct Space {
    position: mint::Vector3<f32>,
    scale: f32,
    orientation: mint::Quaternion<f32>,
}

impl Default for Space {
    fn default() -> Self {
        Self {
            position: mint::Vector3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            scale: 1.0,
            orientation: mint::Quaternion {
                s: 1.0,
                v: mint::Vector3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
            },
        }
    }
}

#[derive(Default, Debug, PartialEq)]
struct Node {
    parent: NodeRef,
    local: Space,
}

#[derive(Default)]
pub struct Scene {
    world: hecs::World,
    nodes: Vec<Node>,
    pub background: Color,
}

impl Scene {
    fn add_node(&mut self, node: Node) -> NodeRef {
        if node.local == Space::default() {
            node.parent
        } else {
            let index = self.nodes.len();
            self.nodes.push(node);
            NodeRef(index as u32)
        }
    }

    pub fn entity(&mut self) -> ObjectBuilder<hecs::EntityBuilder> {
        ObjectBuilder {
            scene: self,
            node: Node::default(),
            kind: hecs::EntityBuilder::new(),
        }
    }
}

pub struct ObjectBuilder<'a, T> {
    scene: &'a mut Scene,
    node: Node,
    kind: T,
}

impl<T> ObjectBuilder<'_, T> {
    pub fn position(mut self, position: mint::Vector3<f32>) -> Self {
        self.node.local.position = position;
        self
    }
}

impl ObjectBuilder<'_, ()> {
    pub fn build(self) -> NodeRef {
        self.scene.add_node(self.node)
    }
}

impl ObjectBuilder<'_, hecs::EntityBuilder> {
    /// Register a new material component with this entity.
    ///
    /// The following components are recognized by the library:
    ///   - [`Color`]
    pub fn component<T: hecs::Component>(mut self, component: T) -> Self {
        self.kind.add(component);
        self
    }

    pub fn build(mut self) -> EntityRef {
        let node = self.scene.add_node(self.node);
        let built = self.kind.add(node).build();
        self.scene.world.spawn(built)
    }
}