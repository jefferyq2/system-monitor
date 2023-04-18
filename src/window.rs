use std::time::{Duration, Instant};

use egui::{FontDefinitions, Style, Visuals};
use egui_wgpu_backend::{RenderPass, ScreenDescriptor};
use egui_winit_platform::{Platform, PlatformDescriptor};
use sysinfo::SystemExt;
use wgpu::{Dx12Compiler, SurfaceError};
use winit::{
    event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::ControlFlow,
    window::{Theme, Window, WindowBuilder},
};

use crate::application::Application;

/// A custom event type for the winit app.
enum EguiEvent {
    RequestRedraw,
}

/// This is the repaint signal type that egui needs for requesting a repaint from another thread.
/// It sends the custom `RequestRedraw` event to the winit event loop.
struct ExampleRepaintSignal(std::sync::Mutex<winit::event_loop::EventLoopProxy<EguiEvent>>);

impl epi::backend::RepaintSignal for ExampleRepaintSignal {
    fn request_repaint(&self) {
        self.0
            .lock()
            .unwrap()
            .send_event(EguiEvent::RequestRedraw)
            .ok();
    }
}

struct State {
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    window: Window,
    platform: Platform,
    egui_rpass: RenderPass,
    application: Application,

    last_updated: Option<Instant>,
}

impl State {
    // Creating some of the wgpu types requires async code
    async fn new(window: Window) -> Self {
        let size = window.inner_size();

        // The instance is a handle to our GPU
        // Backends::all => Vulkan + Metal + DX12 + Browser WebGPU
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            dx12_shader_compiler: Dx12Compiler::default(),
        });

        // # Safety
        //
        // The surface needs to live as long as the window that created it.
        // State owns the window so this should be safe.
        let surface = unsafe { instance.create_surface(&window) }.unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    features: wgpu::Features::default(),
                    // WebGL doesn't support all of wgpu's features, so if
                    // we're building for the web we'll have to disable some.
                    limits: wgpu::Limits::default(),
                    label: None,
                },
                None, // Trace path
            )
            .await
            .unwrap();

        let surface_caps = surface.get_capabilities(&adapter);
        // Shader code in this tutorial assumes an sRGB surface texture. Using a different
        // one will result all the colors coming out darker. If you want to support non
        // sRGB surfaces, you'll need to account for that when drawing to the frame.
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.describe().srgb)
            .unwrap_or(surface_caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &config);

        // We use the egui_winit_platform crate as the platform.
        let platform = Platform::new(PlatformDescriptor {
            physical_width: size.width,
            physical_height: size.height,
            scale_factor: window.scale_factor(),
            font_definitions: FontDefinitions::default(),
            style: Style::default(),
        });

        // We use the egui_wgpu_backend crate as the render backend.
        let egui_rpass = RenderPass::new(&device, surface_format, 1);

        let application = Application::new();

        let last_updated = None;

        Self {
            surface,
            device,
            queue,
            config,
            size,
            window,
            platform,
            egui_rpass,
            application,
            last_updated,
        }
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    #[allow(dead_code)]
    fn input(&mut self, _: &WindowEvent) -> bool {
        false
    }

    fn update(&mut self) {
        self.application.system.refresh_all();

        self.last_updated = Some(Instant::now());
    }

    fn render(&mut self) -> anyhow::Result<(), SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // Begin to draw the UI frame.
        self.platform.begin_frame();

        self.application.render(&self.platform.context());

        // End the UI frame. We could now handle the output and draw the UI with the backend.
        let full_output = self.platform.end_frame(Some(&self.window));
        let paint_jobs = self.platform.context().tessellate(full_output.shapes);

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        // Upload all resources for the GPU.
        let screen_descriptor = ScreenDescriptor {
            physical_width: self.config.width,
            physical_height: self.config.height,
            scale_factor: self.window().scale_factor() as f32,
        };
        let tdelta: egui::TexturesDelta = full_output.textures_delta;
        self.egui_rpass
            .add_textures(&self.device, &self.queue, &tdelta)
            .expect("add texture ok");
        self.egui_rpass
            .update_buffers(&self.device, &self.queue, &paint_jobs, &screen_descriptor);

        // Record all render passes.
        self.egui_rpass
            .execute(
                &mut encoder,
                &view,
                &paint_jobs,
                &screen_descriptor,
                Some(wgpu::Color::BLACK),
            )
            .unwrap();

        // submit will accept anything that implements IntoIter
        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        self.egui_rpass
            .remove_textures(tdelta)
            .expect("remove texture ok");

        Ok(())
    }
}

pub async fn run() -> anyhow::Result<()> {
    let event_loop = winit::event_loop::EventLoopBuilder::<EguiEvent>::with_user_event().build();
    let window = WindowBuilder::new()
        .with_title("System Monitor")
        .with_decorations(true)
        .with_resizable(true)
        .build(&event_loop)?;

    let mut state = State::new(window).await;

    if let Some(theme) = state.window().theme() {
        set_visuals_from_theme(theme, &state);
    }

    event_loop.run(move |event, _, control_flow| {
        // Pass the winit events to the platform integration.
        state.platform.handle_event(&event);
        control_flow.set_wait_timeout(Duration::from_secs(1));

        match event {
            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == state.window().id() => match event {
                WindowEvent::CloseRequested
                | WindowEvent::KeyboardInput {
                    input:
                        KeyboardInput {
                            state: ElementState::Pressed,
                            virtual_keycode: Some(VirtualKeyCode::Escape),
                            ..
                        },
                    ..
                } => *control_flow = ControlFlow::Exit,
                WindowEvent::Resized(physical_size) => {
                    state.resize(*physical_size);
                }
                WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                    // new_inner_size is &&mut so we have to dereference it twice
                    state.resize(**new_inner_size);
                }
                WindowEvent::ThemeChanged(theme) => set_visuals_from_theme(*theme, &state),

                _ => {
                    state.window().request_redraw();
                }
            },
            Event::RedrawRequested(window_id) if window_id == state.window().id() => {
                if let Some(last_updated) = state.last_updated {
                    if last_updated.elapsed() >= Duration::from_secs(1) {
                        state.update();
                    }
                } else {
                    state.update();
                }

                match state.render() {
                    Ok(_) => {}
                    // Reconfigure the surface if lost
                    Err(wgpu::SurfaceError::Lost) => state.resize(state.size),
                    // The system is out of memory, we should probably quit
                    Err(wgpu::SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,
                    // All other errors (Outdated, Timeout) should be resolved by the next frame
                    Err(e) => eprintln!("{e:?}"),
                }
            }
            Event::MainEventsCleared => {
                // RedrawRequested will only trigger once, unless we manually
                // request it.
                if let Some(last_updated) = state.last_updated {
                    if last_updated.elapsed() >= Duration::from_secs(1) {
                        state.window().request_redraw();
                    }
                }
            }
            _ => {}
        }
    });
}

fn set_visuals_from_theme(theme: Theme, state: &State) {
    match theme {
        Theme::Light => state.platform.context().set_visuals(Visuals::light()),
        Theme::Dark => state.platform.context().set_visuals(Visuals::dark()),
    }
}
