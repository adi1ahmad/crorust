# Reference Application Template (`crorust.ruxt`)

## 1. Overview & Scaffolding

`crorust` includes an embedded reference application template located at [`templates/crorust.ruxt`](../templates/crorust.ruxt). It demonstrates production-ready cross-platform graphics, event handling, and windowing across Linux, macOS, Windows, Android, iOS, and WebAssembly.

Scaffolding a new application from the template requires a single command:

```sh
crorust template init my_app
cd my_app
crorust run
```

---

## 2. Technical Architecture

```mermaid
flowchart TB
    %% Styling Classes (Dark Mode & Dual-Theme High Contrast)
    classDef ui fill:#1f3354,stroke:#388bfd,stroke-width:1.5px,color:#e6edf3
    classDef gfx fill:#3d2e14,stroke:#d29922,stroke-width:1.5px,color:#e6edf3
    classDef adapter fill:#163824,stroke:#3fb950,stroke-width:1.5px,color:#e6edf3

    subgraph UIState["Application State & UI"]
        State["AppState<br/><i>Cross-platform settings & UI metrics</i>"]
        Egui["Egui Framework<br/><i>Immediate-mode user interface</i>"]
        State --> Egui
    end

    subgraph Graphics["Graphics Engine"]
        WGPU["WGPU Backend<br/><i>Vulkan, Metal, DirectX 12, WebGPU</i>"]
        Shader["WGSL Shaders<br/><i>Display calibration & triangle rendering</i>"]
        Egui --> WGPU
        Shader --> WGPU
    end

    subgraph PlatformAdapters["Platform Lifecycle Adapters"]
        A_Desk["Desktop Main (src/main.rs)<br/><i>winit event loop</i>"]
        A_And["Android Entry (src/android.rs)<br/><i>GameActivity JNI bindings</i>"]
        A_Web["Web Entry (src/lib.rs)<br/><i>wasm-bindgen & WebGL2 canvas</i>"]
    end

    PlatformAdapters ==> UIState
    PlatformAdapters ==> Graphics

    %% Subgraph Styling
    style UIState fill:none,stroke:#388bfd,stroke-width:1.5px,stroke-dasharray:4 4,color:#e6edf3
    style Graphics fill:none,stroke:#d29922,stroke-width:1.5px,stroke-dasharray:4 4,color:#e6edf3
    style PlatformAdapters fill:none,stroke:#3fb950,stroke-width:1.5px,stroke-dasharray:4 4,color:#e6edf3

    %% Class Assignments
    class State,Egui ui
    class WGPU,Shader gfx
    class A_Desk,A_And,A_Web adapter
```

### The Rendering Pipeline (`egui` + `wgpu`)
The reference application uses immediate-mode GUI with `egui`, rendered through `wgpu`. This provides:
- **Zero Platform-Specific Rendering Code**: The exact same UI code draws widgets on macOS Metal, Windows DirectX 12, Android Vulkan, and Browser WebGPU.
- **Hardware Acceleration**: 60–120 FPS fluid rendering with low CPU overhead.
- **Display Calibration Tools**: Built-in visual tools demonstrating display gamut, DPI scaling, and touch input handling.

---

## 3. Platform Lifecycle Adapters

Different operating systems require fundamentally different process entry points:

### 1. Desktop Platforms (Linux, macOS, Windows)
`src/main.rs` initializes standard logging and launches the `winit` event loop on the main thread:
```rust
fn main() {
    let event_loop = EventLoop::new().expect("Failed to create event loop");
    let mut app = App::new();
    event_loop.run_app(&mut app).expect("Event loop terminated with error");
}
```

### 2. Android (`GameActivity` JNI)
Android processes are created by the zygote and driven by the Java/Kotlin runtime. Rather than using legacy `NativeActivity` (which suffers from keyboard and orientation bugs), `crorust` integrates Google's modern `GameActivity`:
- When Android creates the activity, `android_main` is invoked from a native worker thread.
- Native events (touch pointers, keypresses, window size adjustments) are polled via the Android `ALooper` and translated into `winit` window events.

### 3. WebAssembly (`wasm32-unknown-unknown`)
`src/lib.rs` exports an initialization function marked with `#[wasm_bindgen(start)]`:
- Attaches the `wgpu` rendering surface to an HTML `<canvas id="crorust-canvas">`.
- Binds JavaScript touch and mouse events into the Rust application loop via `web-sys`.

---

## 4. Customizing Your Application

After initializing the template, common customization points include:

1. **Changing Application Name & ID**:
   Edit `manifest.toml` to set your package name, version, and unique reverse-DNS identifier (`com.yourname.app`).
2. **Replacing Application Icons**:
   Place a high-resolution 512x512 PNG at `assets/icon.png`. `crorust` automatically generates all multi-resolution mipmaps and icons during packaging.
3. **Adding 3D Assets or Fonts**:
   Drop assets into the `assets/` folder. They are automatically packaged into the final APK, macOS bundle, or MSIX package.
