use crate::manual::widgets::ManualPlaygroundState;
use egui::{Color32, Frame, RichText, Stroke, Ui};

/// Renders Chapter 1: The Platform Foundation — OS RunLoops & winit.
///
/// Deeply explains:
/// 1. OS Window Servers & The Main-Thread GUI Constraint (Thread 0).
/// 2. winit 0.30 `ApplicationHandler` lifecycle & `ControlFlow` execution modes.
/// 3. Cross-thread wakeups via `EventLoopProxy` (`eventfd`, `PostMessageW`, `CFRunLoopSourceSignal`).
/// 4. Native handles (`RawWindowHandle`) & High-DPI physical-to-logical coordinate systems.
pub fn render_chapter(
    ui: &mut Ui,
    state: &mut ManualPlaygroundState,
    target_to_launch: &mut Option<&'static str>,
) {
    // -------------------------------------------------------------------------
    // Chapter Title & Subtitle Header
    // -------------------------------------------------------------------------
    ui.heading(
        RichText::new("🏛️ Chapter 1: The Platform Foundation — OS RunLoops & winit")
            .size(22.0)
            .strong(),
    );
    ui.add_space(4.0);
    ui.label(
        RichText::new(
            "Every cross-platform graphical application rests upon a hardware abstraction layer that bridges \
             the operating system's asynchronous interrupt queues to user space. This chapter explores how \
             winit 0.30 normalizes window servers, enforces the Thread 0 GUI constraint, manages mobile \
             surface lifetimes, and coordinates cross-thread execution via EventLoopProxy.",
        )
        .size(13.5)
        .color(ui.visuals().weak_text_color()),
    );
    ui.add_space(12.0);
    ui.separator();
    ui.add_space(12.0);

    // -------------------------------------------------------------------------
    // Architectural Diagram: Event Loop & Thread Wakeup Pipeline
    // -------------------------------------------------------------------------
    Frame::group(ui.style())
        .fill(if ui.visuals().dark_mode {
            Color32::from_rgb(15, 23, 42)
        } else {
            Color32::from_rgb(245, 247, 250)
        })
        .stroke(Stroke::new(1.0, ui.visuals().window_stroke().color))
        .inner_margin(12.0)
        .corner_radius(8.0)
        .show(ui, |ui| {
            ui.label(
                RichText::new("📊 Architectural Overview: The OS Event Pump & Worker Bridge")
                    .strong(),
            );
            ui.add_space(6.0);
            let viewer = crate::manual::widgets::diagram_viewer::InteractiveDiagramViewer::new(
                "ch01_event_loop",
                egui::include_image!("../../../assets/manual/svg/ch01_event_loop.svg"),
                include_str!("../../../assets/manual/diagrams/ch01_event_loop.mmd"),
                egui::vec2(1381.78, 2162.45),
            );
            viewer.show(ui, state.diagram_state_mut("ch01_event_loop"));
            ui.add_space(4.0);
            ui.label(
                RichText::new(
                    "Figure 1.1: Native OS runloops push physical events to winit's EventLoop on Thread 0, \
                     while background worker threads inject synthetic events via EventLoopProxy through OS kernel primitives.",
                )
                .small()
                .weak(),
            );
        });

    ui.add_space(18.0);

    // -------------------------------------------------------------------------
    // Subsection 1.1: The Role of winit
    // -------------------------------------------------------------------------
    ui.heading(RichText::new("1.1 The Role of winit: Hardware & OS Abstraction Layer").size(17.0));
    ui.add_space(4.0);
    ui.label(
        "Sequential command-line utilities execute synchronously: `main()` begins, reads standard input or arguments, \
         computes a result, and terminates. Graphical applications, by contrast, are fundamentally interrupt-driven. \
         They do not execute in a tight loop; instead, the application process parks its thread inside kernel wait \
         primitives (such as `epoll_wait(2)`, `GetMessageW`, or `CFRunLoopRunInMode`) and consumes 0.0% CPU until a hardware \
         interrupt or window server event arrives.",
    );
    ui.add_space(8.0);

    egui::Frame::group(ui.style())
        .fill(ui.visuals().faint_bg_color)
        .inner_margin(10.0)
        .corner_radius(6.0)
        .show(ui, |ui| {
            ui.label(RichText::new("🖥️ Cross-Platform Window Servers at a Glance:").strong());
            ui.add_space(4.0);
            ui.label(
                "• Windows (Win32): Managed by win32k.sys. Each GUI thread has a message queue. Messages are \
                 retrieved via `GetMessageW(&msg, NULL, 0, 0)`, translated with `TranslateMessage(&msg)`, and dispatched \
                 to the registered window procedure callback (`WNDPROC`) via `DispatchMessageW(&msg)`.\n\
                 • macOS (Cocoa): AppKit applications run an `NSRunLoop` inside `NSApplication`. Mach port messages \
                 are translated into Objective-C `NSEvent` instances and routed down the responder chain (`NSWindow` -> `NSView`).\n\
                 • Linux (Wayland): A modern asynchronous IPC protocol over a Unix domain socket (`$XDG_RUNTIME_DIR/wayland-0`). \
                 Clients negotiate surface roles (`xdg_surface`, `xdg_toplevel`) with the compositor and swap shared memory \
                 (`wl_shm`) or GPU DMA-BUFs. Loop execution occurs via `wl_event_queue_dispatch()`.\n\
                 • Linux (X11): Network socket IPC over `/tmp/.X11-unix/X0`. Clients send X11 protocol packets and poll events \
                 via `xcb_wait_for_event()` or `XNextEvent()`.\n\
                 • Android (NDK): Native apps use `android_native_app_glue`. The Linux kernel event queue is polled via \
                 `ALooper_pollAll()` or `ALooper_pollOnce()`. The native window is represented by `ANativeWindow*`.\n\
                 • iOS (UIKit): Driven by `UIApplicationMain` running a `CFRunLoop` on the main Mach port.\n\
                 • Web (WASM): WebAssembly cannot block the thread. The browser owns the event loop, dispatching DOM events \
                 (`pointerdown`, `keydown`) and invoking Rust callbacks via cooperative `requestAnimationFrame` yielding."
            );
        });

    ui.add_space(10.0);
    ui.label(RichText::new("The Strict Main-Thread GUI Constraint (Thread 0 Invariant):").strong());
    ui.label(
        "Operating systems enforce strict thread affinity for window management. On macOS, AppKit asserts that \
         `[NSApp run]` and all UI mutations occur on the thread that initialized `NSApplication` (Thread 0); invoking AppKit \
         from a worker thread triggers an instant process abort (`NSInternalInconsistencyException`). On Windows, Win32 \
         message queues are thread-bound: `GetMessageW` only retrieves messages posted to windows created by the calling thread. \
         In WebAssembly, DOM access and WebGL contexts are strictly single-threaded. Therefore, winit must own and execute the \
         event loop exclusively on the main operating system thread.",
    );

    ui.add_space(12.0);
    crate::rust_view_ui(
        ui,
        r#"// Minimal winit 0.30 event loop initialization on Thread 0
use winit::event_loop::{ControlFlow, EventLoop};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Create the platform EventLoop on Thread 0
    let event_loop = EventLoop::new()?;

    // 2. ControlFlow::Wait parks the thread in kernel primitives (0% CPU)
    event_loop.set_control_flow(ControlFlow::Wait);

    // 3. Run application with an ApplicationHandler implementation
    let mut app = RuxtAppHandler::default();
    event_loop.run_app(&mut app)?;
    Ok(())
}"#,
    );

    ui.add_space(18.0);

    // -------------------------------------------------------------------------
    // Subsection 1.2: Anatomy of the Event Loop
    // -------------------------------------------------------------------------
    ui.heading(
        RichText::new("1.2 Anatomy of the Event Loop (EventLoop & ApplicationHandler)").size(17.0),
    );
    ui.add_space(4.0);
    ui.label(
        "winit 0.30 replaced the legacy closure-based `event_loop.run(...)` API with the structured \
         `winit::application::ApplicationHandler` trait. This redesign cleanly reflects native OS lifecycles, \
         particularly on mobile platforms (Android and iOS).",
    );
    ui.add_space(8.0);

    egui::Frame::group(ui.style())
        .fill(ui.visuals().faint_bg_color)
        .inner_margin(10.0)
        .corner_radius(6.0)
        .show(ui, |ui| {
            ui.label(RichText::new("🔄 Lifecycle Methods of ApplicationHandler:").strong());
            ui.add_space(4.0);
            ui.label(
                "1. `resumed(&mut self, event_loop: &ActiveEventLoop)`:\n\
                   Called when the OS grants a display surface. This is where windows and GPU surfaces must be created.\n\
                   ⚠️ Deep Mobile Systems Invariant: On Android and iOS, `resumed` is called repeatedly when waking from sleep \
                   or task-switching. The operating system destroys the underlying `ANativeWindow` surface when backgrounded; \
                   applications must recreate or rebind their GPU swapchains (`wgpu::Surface`) inside `resumed`.\n\n\
                 2. `window_event(&mut self, event_loop: &ActiveEventLoop, window_id: WindowId, event: WindowEvent)`:\n\
                   Targeted window notifications: resizing, scale factor changes, pointer movements, touch points, and close requests.\n\n\
                 3. `about_to_wait(&mut self, event_loop: &ActiveEventLoop)`:\n\
                   The resting state. Called when all pending OS messages have been processed. This is where the application \
                   decides whether to sleep indefinitely, schedule a timed wakeup, or request an immediate frame redraw.\n\n\
                 4. `suspended(&mut self, event_loop: &ActiveEventLoop)`:\n\
                   Mobile teardown callback (`APP_CMD_TERM_WINDOW` on Android). Applications MUST drop their GPU surface handles \
                   immediately, or risk graphics driver segmentation faults and OS termination."
            );
        });

    ui.add_space(10.0);
    ui.label(RichText::new("The Three ControlFlow Execution Modes:").strong());
    ui.label(
        "• `ControlFlow::Wait`: The event loop sleeps in the OS kernel until an external hardware or system event arrives. \
         CPU utilization is 0.0%, preserving battery life and eliminating fan noise in desktop utilities.\n\
         • `ControlFlow::Poll`: Continuously spins the loop without parking. Useful only for raw benchmark loops or dedicated \
         game engines, but exhausts battery and pins a CPU core at 100%.\n\
         • `ControlFlow::WaitUntil(Instant)`: Paced wakeups. Wakes automatically when an animation deadline arrives \
         (e.g., 16.6ms for 60 FPS transitions), then returns to sleep.",
    );

    ui.add_space(12.0);
    crate::rust_view_ui(
        ui,
        r#"use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowId};

#[derive(Default)]
pub struct RuxtAppHandler {
    window: Option<Window>,
}

impl ApplicationHandler for RuxtAppHandler {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // Surface creation point: must support recreation on mobile wake
        if self.window.is_none() {
            let attrs = Window::default_attributes().with_title("Ruxt Application");
            self.window = Some(event_loop.create_window(attrs).unwrap());
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::RedrawRequested => {
                if let Some(window) = &self.window {
                    // Frame rendering pulse executed here
                    window.request_redraw();
                }
            }
            _ => (),
        }
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        // Drop native surface on Android to prevent driver crashes
        self.window = None;
    }
}"#,
    );

    ui.add_space(18.0);

    // -------------------------------------------------------------------------
    // Subsection 1.3: Asynchronous Communication with EventLoopProxy
    // -------------------------------------------------------------------------
    ui.heading(RichText::new("1.3 Asynchronous Communication with EventLoopProxy").size(17.0));
    ui.add_space(4.0);
    ui.label(
        "Modern applications perform heavy operations off the main thread: Tokio asynchronous networking, \
         image decodes, database queries, and audio synthesis. Because GUI mutations are restricted to Thread 0, \
         worker threads must communicate results across thread boundaries.",
    );
    ui.add_space(8.0);

    ui.label(RichText::new("The Thread Crossing Problem & Channel Deadlock Trap:").strong());
    ui.label(
        "Why can't worker threads simply push data into a standard `std::sync::mpsc` channel? \
         Because if the UI thread is parked inside `epoll_wait(2)` or `GetMessageW` at 0% CPU, pushing a message into \
         a standard memory channel will NOT wake the kernel! The message sits in the queue indefinitely until the user \
         physically wiggles the mouse to generate an OS interrupt. \
         To wake a sleeping event loop, the worker thread must trigger an operating system interrupt primitive.",
    );
    ui.add_space(8.0);

    egui::Frame::group(ui.style())
        .fill(ui.visuals().faint_bg_color)
        .inner_margin(10.0)
        .corner_radius(6.0)
        .show(ui, |ui| {
            ui.label(
                RichText::new("⚡ How EventLoopProxy<T> Wakes the Kernel Across Platforms:")
                    .strong(),
            );
            ui.add_space(4.0);
            ui.label(
                "An `EventLoopProxy<T>` is a thread-safe (`Send + Sync`), cloneable multi-producer handle that \
                 injects custom user events directly into the platform event pump:\n\
                 • Linux (Wayland/X11): The proxy writes an 8-byte integer counter to an `eventfd(2)` or pipe descriptor \
                 monitored by the main thread's `epoll_wait(2)` call. The kernel unblocks the thread immediately.\n\
                 • Windows (Win32): The proxy calls `PostMessageW(hwnd, WM_USER + X, wparam, lparam)`, placing a message \
                 directly into the thread's message queue and unblocking `GetMessageW`.\n\
                 • macOS (Cocoa): The proxy invokes `CFRunLoopSourceSignal(source)` followed by `CFRunLoopWakeUp(main_runloop)`, \
                 which sends an asynchronous Mach message interrupt to `mach_msg_trap`."
            );
        });

    ui.add_space(12.0);
    crate::rust_view_ui(
        ui,
        r#"// Background worker pattern with EventLoopProxy
use winit::event_loop::EventLoopProxy;

#[derive(Debug, Clone)]
pub enum UserEvent {
    DownloadProgress(f32),
    DataReady(Vec<u8>),
}

pub fn spawn_network_task(proxy: EventLoopProxy<UserEvent>, url: String) {
    std::thread::spawn(move || {
        for progress in [0.25, 0.50, 0.75, 1.0] {
            std::thread::sleep(std::time::Duration::from_millis(200));
            // Wakes the sleeping UI thread through kernel primitives:
            let _ = proxy.send_event(UserEvent::DownloadProgress(progress));
        }
        let payload = vec![0xDE, 0xAD, 0xBE, 0xEF];
        let _ = proxy.send_event(UserEvent::DataReady(payload));
    });
}"#,
    );

    ui.add_space(18.0);

    // -------------------------------------------------------------------------
    // Subsection 1.4: Native Surfaces & Display Handles
    // -------------------------------------------------------------------------
    ui.heading(
        RichText::new("1.4 Native Surfaces & Display Handles (raw-window-handle)").size(17.0),
    );
    ui.add_space(4.0);
    ui.label(
        "To initialize a hardware graphics pipeline (`wgpu`, `vulkano`, or `glow`), the rendering backend must acquire \
         the underlying operating system window pointers. The `raw-window-handle` crate provides type-safe Rust wrappers \
         around platform-specific handles without pulling in entire platform SDK headers.",
    );
    ui.add_space(8.0);

    egui::Frame::group(ui.style())
        .fill(ui.visuals().faint_bg_color)
        .inner_margin(10.0)
        .corner_radius(6.0)
        .show(ui, |ui| {
            ui.label(RichText::new("📐 Physical Pixels vs. Logical Points:").strong());
            ui.add_space(4.0);
            ui.label(
                "Modern displays vary wildly in pixel density, from 96 DPI standard desktop monitors to 280+ DPI 4K Retina panels. \
                 If GUI buttons were sized in raw physical pixels, a 100-pixel button that looks perfect on 1080p would appear \
                 microscopically small on a 4K display.\n\n\
                 The High-DPI Normalization Formula:\n\
                 Logical Points = Physical Pixels / Scale Factor\n\n\
                 When a window is dragged between a standard monitor (Scale Factor 1.0) and a 4K display (Scale Factor 2.0), \
                 winit emits `WindowEvent::ScaleFactorChanged`. The application adjusts its internal DPI ratio while keeping \
                 logical layout coordinates constant."
            );
        });

    ui.add_space(10.0);
    ui.label(RichText::new("Wayland vs. X11 Window Placement Security Boundaries:").strong());
    ui.label(
        "In X11, clients have global desktop coordinate awareness. Any application can query the global mouse position, \
         warp the cursor, position floating windows anywhere on the screen, or inspect keyboard input globally. \
         Wayland radically restricts this model for security: clients only know their own local surface coordinates \
         `(0, 0) .. (width, height)` and have no knowledge of monitor bounds or peer windows. This prevents malicious \
         clickjacking, keystroke logging, and screen snooping.",
    );

    ui.add_space(12.0);
    crate::rust_view_ui(
        ui,
        r#"use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

fn inspect_native_handles(window: &winit::window::Window) {
    // Acquire safe, typed FFI handles to native window server primitives
    let window_handle = window.window_handle().expect("Valid window handle");
    let display_handle = window.display_handle().expect("Valid display handle");

    println!("Window handle: {:?}", window_handle.as_raw());
    println!("Display handle: {:?}", display_handle.as_raw());
    println!("DPI Scale factor: {:.2}", window.scale_factor());
}"#,
    );

    ui.add_space(20.0);
    ui.separator();
    ui.add_space(12.0);

    // -------------------------------------------------------------------------
    // Chapter Navigation Footer
    // -------------------------------------------------------------------------
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("Completed Chapter 1.")
                .strong()
                .color(Color32::from_rgb(16, 185, 129)),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .button(RichText::new("Explore Showcase Windows ➔").strong())
                .clicked()
            {
                *target_to_launch = Some("📦 Widget Showcase");
            }
        });
    });
}
