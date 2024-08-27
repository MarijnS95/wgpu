use std::borrow::Cow;
use wgpu::rwh::{HasWindowHandle, RawWindowHandle, Win32WindowHandle};
use windows::{
    core::Interface as _,
    Win32::{
        Foundation::HWND,
        Graphics::{CompositionSwapchain, DirectComposition, Dxgi},
    },
};
use winit::{
    event::{Event, WindowEvent},
    event_loop::EventLoop,
    window::Window,
};

const USE_HANDLE: bool = true;

async fn run(event_loop: EventLoop<()>, window: Window) {
    let mut size = window.inner_size();
    size.width = size.width.max(1);
    size.height = size.height.max(1);

    let instance = wgpu::Instance::default();

    let dev: DirectComposition::IDCompositionDevice = // Or DesktopDevice
        unsafe { DirectComposition::DCompositionCreateDevice2(None) }.unwrap();
    dbg!(&dev);

    let ddev: DirectComposition::IDCompositionDeviceDebug = dev.cast().unwrap();
    unsafe { ddev.EnableDebugCounters() }.unwrap();

    let RawWindowHandle::Win32(Win32WindowHandle { hwnd, .. }) =
        window.window_handle().unwrap().as_raw()
    else {
        panic!()
    };
    let composition_target =
        unsafe { dev.CreateTargetForHwnd(HWND(hwnd.get() as *mut _), true) }.unwrap();
    dbg!(&composition_target);

    let surf_hnd = unsafe {
        DirectComposition::DCompositionCreateSurfaceHandle(
            3, // COMPOSITIONSURFACE_ALL_ACCESS
            None,
        )
    }
    .unwrap();
    dbg!(surf_hnd);

    let main_visual = unsafe { dev.CreateVisual() }.unwrap();
    unsafe { composition_target.SetRoot(&main_visual) }.unwrap();

    let surface = if !USE_HANDLE {
        unsafe {
            instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::CompositionVisual(
                main_visual.as_raw(),
            ))
        }
        .unwrap()
    } else {
        let render_surf = unsafe { dev.CreateSurfaceFromHandle(surf_hnd) }.unwrap();
        dbg!(&render_surf);
        unsafe { main_visual.SetContent(&render_surf) }.unwrap();

        let surface = unsafe {
            instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::SurfaceHandle(surf_hnd.0))
        }
        .unwrap();
        dbg!(&surface);

        surface
    };

    // main_visual.SetCompositeMode(DirectComposition::DCOMPOSITION_COMPOSITE_MODE_DESTINATION_INVERT);

    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            force_fallback_adapter: false,
            // Request an adapter which can render to our surface
            compatible_surface: Some(&surface),
        })
        .await
        .expect("Failed to find an appropriate adapter");

    // Create the logical device and command queue
    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::empty(),
                // Make sure we use the texture resolution limits from the adapter, so we can support images the size of the swapchain.
                required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                    .using_resolution(adapter.limits()),
                memory_hints: wgpu::MemoryHints::MemoryUsage,
            },
            None,
        )
        .await
        .expect("Failed to create device");

    // Load the shaders from disk
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("shader.wgsl"))),
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[],
        push_constant_ranges: &[],
    });

    let swapchain_capabilities = surface.get_capabilities(&adapter);
    let swapchain_format = swapchain_capabilities.formats[0];

    let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None,
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(swapchain_format.into())],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    });

    let mut config = surface
        .get_default_config(&adapter, size.width, size.height)
        .unwrap();

    surface.configure(&device, &config);

    // let factory: CompositionSwapchain::IPresentationFactory = unsafe {
    //     device.as_hal::<wgpu::hal::dx12::Api, _, _>(|device| {
    //         dbg!(device.unwrap().raw_device().cast::<Dxgi::IDXGIDevice>());
    //         CompositionSwapchain::CreatePresentationFactory(
    //             // dbg!(device.unwrap().raw_queue()),
    //             dbg!(device.unwrap().raw_device()),
    //         )
    //     })
    // }
    // .unwrap()
    // .unwrap();
    // dbg!(factory);

    // let factory: CompositionSwapchain::IPresentationFactory = unsafe {
    //     adapter.as_hal::<wgpu::hal::dx12::Api, _, _>(|adapter| {
    //         CompositionSwapchain::CreatePresentationFactory(dbg!(&**adapter.unwrap().raw_adapter()))
    //     })
    // }
    // .unwrap();
    // dbg!(factory);

    let mut rendered = false;
    let window = &window;
    event_loop
        .run(move |event, target| {
            // Have the closure take ownership of the resources.
            // `event_loop.run` never returns, therefore we must do this to ensure
            // the resources are properly cleaned up.
            let _ = (&instance, &adapter, &shader, &pipeline_layout);

            if let Event::WindowEvent {
                window_id: _,
                event,
            } = event
            {
                match event {
                    WindowEvent::Resized(new_size) => {
                        // Reconfigure the surface with the new size
                        config.width = new_size.width.max(1);
                        config.height = new_size.height.max(1);
                        surface.configure(&device, &config);
                        // On macos the window needs to be redrawn manually after resizing
                        // window.request_redraw();
                    }
                    WindowEvent::RedrawRequested => {
                        // if rendered {
                        //     // TODO: Play with scaling?
                        //     unsafe { dev.Commit() }.unwrap();
                        //     return;
                        // }
                        // rendered = true;
                        let frame = surface
                            .get_current_texture()
                            .expect("Failed to acquire next swap chain texture");
                        let view = frame
                            .texture
                            .create_view(&wgpu::TextureViewDescriptor::default());
                        let mut encoder =
                            device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                                label: None,
                            });
                        {
                            let mut rpass =
                                encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                                    label: None,
                                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                        view: &view,
                                        resolve_target: None,
                                        ops: wgpu::Operations {
                                            load: wgpu::LoadOp::Clear(wgpu::Color::GREEN),
                                            store: wgpu::StoreOp::Store,
                                        },
                                    })],
                                    depth_stencil_attachment: None,
                                    timestamp_writes: None,
                                    occlusion_query_set: None,
                                });
                            rpass.set_pipeline(&render_pipeline);
                            rpass.draw(0..3, 0..1);
                        }

                        queue.submit(Some(encoder.finish()));
                        frame.present();

                        unsafe { dev.Commit() }.unwrap();
                        // unsafe { dev.WaitForCommitCompletion() }.unwrap();
                        dbg!(unsafe { dev.GetFrameStatistics() }.unwrap());
                        for fid in [
                            DirectComposition::COMPOSITION_FRAME_ID_COMPLETED,
                            DirectComposition::COMPOSITION_FRAME_ID_CREATED,
                            DirectComposition::COMPOSITION_FRAME_ID_CONFIRMED,
                        ] {
                            // TODO: Skip if the ID for created and confirmed is the same.
                            let c =
                                dbg!(unsafe { DirectComposition::DCompositionGetFrameId(fid) }
                                    .unwrap());
                            let mut framestats = Default::default();
                            let mut ids = Vec::with_capacity(20);
                            let mut actual = 0;
                            unsafe {
                                DirectComposition::DCompositionGetStatistics(
                                    c,
                                    &mut framestats,
                                    // Note that we can also query 0/null() targets and let this function
                                    // only return how many items there would be, to allocate the Vec.
                                    ids.capacity() as u32,
                                    Some(ids.as_mut_ptr()),
                                    Some(&mut actual),
                                )
                            }
                            .unwrap();
                            // dbg!(actual);
                            unsafe { ids.set_len(actual as usize) };
                            dbg!(framestats);
                            // dbg!(&ids);
                            for id in &ids {
                                dbg!(unsafe {
                                    DirectComposition::DCompositionGetTargetStatistics(c, id)
                                }
                                .unwrap());
                            }
                        }
                    }
                    WindowEvent::CloseRequested => target.exit(),
                    _ => {}
                };
            }
        })
        .unwrap();
}

pub fn main() {
    let event_loop = EventLoop::new().unwrap();
    #[allow(unused_mut)]
    let mut builder = winit::window::WindowBuilder::new();
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::JsCast;
        use winit::platform::web::WindowBuilderExtWebSys;
        let canvas = web_sys::window()
            .unwrap()
            .document()
            .unwrap()
            .get_element_by_id("canvas")
            .unwrap()
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .unwrap();
        builder = builder.with_canvas(Some(canvas));
    }
    let window = builder.build(&event_loop).unwrap();

    #[cfg(not(target_arch = "wasm32"))]
    {
        env_logger::init();
        pollster::block_on(run(event_loop, window));
    }
    #[cfg(target_arch = "wasm32")]
    {
        std::panic::set_hook(Box::new(console_error_panic_hook::hook));
        console_log::init().expect("could not initialize logger");
        wasm_bindgen_futures::spawn_local(run(event_loop, window));
    }
}
