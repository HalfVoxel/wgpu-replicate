use async_std::task;
use wgpu::{CommandEncoderDescriptor, Device};
use winit::{dpi::PhysicalSize, event::{Event, WindowEvent}, event_loop::{ControlFlow, EventLoop}, window::Window};

const DEFAULT_WINDOW_WIDTH: u32 = 2048;
const DEFAULT_WINDOW_HEIGHT: u32 = 2048;

/// Creates a texture that uses MSAA and fits a given swap chain
fn create_multisampled_framebuffer(
    device: &wgpu::Device,
    size: &wgpu::Extent3d,
    sample_count: u32,
    format: wgpu::TextureFormat,
) -> Texture {
    Texture::new(
        device,
        wgpu::TextureDescriptor {
            size: wgpu::Extent3d {
                width: size.width,
                height: size.height,
                depth: 1,
            },
            mip_level_count: 1,
            // array_layer_count: 1,
            sample_count,
            dimension: wgpu::TextureDimension::D2,
            format, //sc_desc.format,
            usage: wgpu::TextureUsage::RENDER_ATTACHMENT,
            label: Some("MSAA Framebuffer"),
        },
    )
}

pub struct Texture {
    pub descriptor: wgpu::TextureDescriptor<'static>,
    pub buffer: wgpu::Texture,
    pub view: wgpu::TextureView,
}

impl Texture {
    pub fn new(device: &Device, descriptor: wgpu::TextureDescriptor) -> Texture {
        let tex = device.create_texture(&descriptor);

        // Remove the label which we do not have a static lifetime for
        let descriptor = wgpu::TextureDescriptor::<'static> {
            label: None,
            size: descriptor.size,
            // array_layer_count: descriptor.array_layer_count,
            mip_level_count: descriptor.mip_level_count,
            sample_count: descriptor.sample_count,
            dimension: descriptor.dimension,
            format: descriptor.format,
            usage: descriptor.usage,
        };

        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());

        Texture {
            descriptor,
            buffer: tex,
            view,
        }
    }
}

fn main() {
    println!("Hello, world!");

    let instance = wgpu::Instance::new(wgpu::BackendBit::PRIMARY);
    let adapter = task::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        compatible_surface: None, // TODO
    }))
    .unwrap();

    let (device, queue) = task::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: None,
            features: wgpu::Features::NON_FILL_POLYGON_MODE
                | wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES,
            limits: wgpu::Limits::default(),
        },
        Some(&std::path::Path::new("trace")),
    ))
    .expect("Failed to request device");

    let event_loop = EventLoop::new();
    let window = Window::new(&event_loop).unwrap();
    window.set_inner_size(PhysicalSize::new(
        DEFAULT_WINDOW_WIDTH,
        DEFAULT_WINDOW_HEIGHT,
    ));
    let size = window.inner_size();

    let mut swap_chain_desc = wgpu::SwapChainDescriptor {
        usage: wgpu::TextureUsage::RENDER_ATTACHMENT,
        format: wgpu::TextureFormat::Bgra8Unorm,
        width: size.width,
        height: size.height,
        present_mode: wgpu::PresentMode::Fifo,
    };

    let window_surface = unsafe { instance.create_surface(&window) };
    let mut swap_chain = device.create_swap_chain(&window_surface, &swap_chain_desc);

    let sample_count = 8;

    let window_extent = wgpu::Extent3d {
        width: swap_chain_desc.width,
        height: swap_chain_desc.height,
        depth: 1,
    };
    
    let mut multisample_texture = create_multisampled_framebuffer(
        &device,
        &window_extent,
        sample_count,
        swap_chain_desc.format,
    );

    let mut init_encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Init encoder"),
    });
    queue.submit(std::iter::once(init_encoder.finish()));

    event_loop.run(move |event, _, control_flow| {

        let mut rebuild_swapchain = false;
        match event {
            Event::MainEventsCleared => {
            }
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::Destroyed | WindowEvent::CloseRequested => {
                    *control_flow = ControlFlow::Exit;
                }
                WindowEvent::Resized(size) => {
                    rebuild_swapchain = true;
                    swap_chain_desc.width = size.width;
                    swap_chain_desc.height = size.height;
                }
                WindowEvent::ScaleFactorChanged { .. } => {
                    rebuild_swapchain = true;
                    println!("DPI changed");
                }
                _ => {}
            }
            Event::RedrawRequested(_) => {
                let swapchain_output = swap_chain.get_current_frame().unwrap();

                let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
                    label: Some("Frame encoder"),
                });

                let framebuffer_target = &swapchain_output.output.view;
                let multisample_target = &multisample_texture.view;

                encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("some msaa pass"),
                    color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                        attachment: multisample_target,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                            store: true,
                        },
                        resolve_target: None,
                    }],
                    depth_stencil_attachment: None,
                });

                encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("resolve pass pass"),
                    color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                        attachment: multisample_target,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: true,
                        },
                        resolve_target: Some(framebuffer_target),
                    }],
                    depth_stencil_attachment: None,
                });

                encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("non-msaa pass"),
                    color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                        attachment: framebuffer_target,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: true,
                        },
                        resolve_target: None,
                    }],
                    depth_stencil_attachment: None,
                });

                queue.submit(std::iter::once(encoder.finish()));
            }
            _ => {}
        }

        let window_extent = wgpu::Extent3d {
            width: swap_chain_desc.width,
            height: swap_chain_desc.height,
            depth: 1,
        };

        if rebuild_swapchain {
            println!("Rebuilding swap chain");
            swap_chain = device.create_swap_chain(&window_surface, &swap_chain_desc);
            multisample_texture = create_multisampled_framebuffer(
                &device,
                &window_extent,
                sample_count,
                swap_chain_desc.format,
            );
        }

        
    });
}
