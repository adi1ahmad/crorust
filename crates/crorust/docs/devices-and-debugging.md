# Target Devices & Interactive Debugging

## 1. Device Discovery Architecture

`crorust` connects your development workstation directly to target hardware, simulators, and local execution environments. Running `crorust devices` enumerates all available destinations:

```sh
crorust devices
```

Example output:
```text
host                                              Linux               linux x64           Arch Linux 5.16.10-arch1-1
adb:16ee50bc                                      FP4                 android arm64       Android 11 (API 30)
imd:55abbd4b70af4353bdea2595bbddcac4a2b7891a      David’s iPhone      ios arm64           iPhone OS 15.3.1
sim:D9A01B2E-3184-4C2E-8A55-9B04D12FE3A1          iPhone 15 Pro       ios arm64           iOS 17.4 (Simulator)
```

### Discovery Protocols Under the Hood

```mermaid
flowchart TD
    %% Styling Classes (Dark Mode & Dual-Theme High Contrast)
    classDef host fill:#1f3354,stroke:#388bfd,stroke-width:1.5px,color:#e6edf3
    classDef bridge fill:#3d2e14,stroke:#d29922,stroke-width:1.5px,color:#e6edf3
    classDef target fill:#163824,stroke:#3fb950,stroke-width:1.5px,color:#e6edf3

    subgraph HostWorkstation["Host Workstation"]
        CLI["crorust devices"]
    end

    subgraph Bridges["Discovery Bridges & Protocols"]
        B_Adb["Android ADB<br/><i>adb devices -l</i>"]
        B_Imd["Apple usbmuxd / libimobiledevice<br/><i>idevice_id -l</i>"]
        B_Sim["Apple Simulator simctl<br/><i>xcrun simctl list devices</i>"]
        B_Host["Local Host Engine<br/><i>uname / OS kernel query</i>"]
    end

    subgraph Targets["Target Devices"]
        D_And["Android Phones & Emulators"]
        D_iOS["Physical iPhones & iPads"]
        D_Sim["iOS Simulators (macOS only)"]
        D_Loc["Workstation Desktop"]
    end

    CLI --> B_Adb & B_Imd & B_Sim & B_Host
    B_Adb --> D_And
    B_Imd --> D_iOS
    B_Sim --> D_Sim
    B_Host --> D_Loc

    %% Subgraph Styling
    style HostWorkstation fill:none,stroke:#388bfd,stroke-width:1.5px,stroke-dasharray:4 4,color:#e6edf3
    style Bridges fill:none,stroke:#d29922,stroke-width:1.5px,stroke-dasharray:4 4,color:#e6edf3
    style Targets fill:none,stroke:#3fb950,stroke-width:1.5px,stroke-dasharray:4 4,color:#e6edf3

    %% Class Assignments
    class CLI host
    class B_Adb,B_Imd,B_Sim,B_Host bridge
    class D_And,D_iOS,D_Sim,D_Loc target
```

1. **Android Physical Hardware & Emulators (`adb:<id>`)**:
   `AndroidDebugBridgeController` queries `adb devices -l`, parsing product identifiers, models, and CPU architectures (`ro.product.cpu.abi`).
2. **Apple Physical Hardware (`imd:<udid>`)**:
   Communicates over USB via `usbmuxd` (the multiplexed daemon running on port 27015). Works on Linux, macOS, and Windows.
3. **Apple iOS Simulator (`sim:<uuid>`)**:
   Queries `xcrun simctl` on macOS hosts to find booted or available iOS simulator runtimes.
4. **Local Host (`host`)**:
   Detects current host operating system and CPU architecture for instant desktop execution.

---

## 2. Deploy & Run Workflow (`crorust run`)

Passing `--device <id>` tells `crorust` to automatically infer the target platform and architecture:

```sh
# Deploy and stream logs from an Android phone
crorust run --device adb:16ee50bc

# Deploy to an iOS Simulator
crorust run --device sim:D9A01B2E-3184-4C2E-8A55-9B04D12FE3A1
```

### Execution Steps
1. **Target Architecture Inference**: Queries the device to determine whether it is 64-bit ARM (`arm64`) or x86_64 (`x64`).
2. **Build & Package**: Triggers Cargo compilation and format bundling (`.apk`, `.app`).
3. **Installation**: Transfers the package over the bridge protocol (`adb install -r` or `ideviceinstaller`).
4. **Activity Launch**: Triggers the entry point activity (`am start -n <package>/<activity>`).
5. **Log Streaming**: Attaches to `logcat` (filtering on the application's process ID) or stdout, streaming clean output to your terminal with automatic process cleanup on `Ctrl+C`.

---

## 3. Interactive Remote LLDB Debugging (`crorust debug`)

When diagnosing memory corruption, segfaults, or panic backtraces, printf-style debugging is inefficient. `crorust debug` provisions an interactive LLDB debugging session:

```mermaid
sequenceDiagram
    autonumber
    actor Dev as Developer Terminal
    participant CLI as crorust debug --device <id>
    participant Bridge as ADB / usbmuxd Tunnel
    participant Device as Target Device
    participant Server as lldb-server / debugserver
    participant LLDB as Host LLDB Client

    Dev->>CLI: crorust debug --device adb:16ee50bc
    CLI->>Device: Install debug package & configure wait-for-debugger
    CLI->>Bridge: Push or locate lldb-server on device
    CLI->>Device: Launch lldb-server platform on local domain socket
    CLI->>Bridge: Forward target socket port to localhost:5039
    CLI->>Device: Launch application process (halts at entry)
    CLI->>LLDB: Launch host LLDB with application symbols
    LLDB->>Bridge: Connect to localhost:5039 (gdb-remote protocol)
    Bridge->>Server: Handshake established
    Server-->>LLDB: Attached to process!
    CLI->>Dev: Interactive LLDB REPL (breakpoints, backtraces, memory inspection)
```

### Android Debugging Mechanics
1. `crorust` locates `lldb-server` inside the Android NDK staged in `.cache/crorust/`.
2. It pushes `lldb-server` to `/data/local/tmp/` on the device and gives it execute permissions (`chmod 755`).
3. It launches `lldb-server` listening on a UNIX domain socket.
4. It invokes `adb forward tcp:5039 localabstract:lldb-server` to tunnel packets over USB.
5. It drops the developer into the standard LLDB prompt with full source line mappings.

### iOS Debugging Mechanics
1. On physical iPhones, `crorust` mounts the Apple Developer Disk Image (`DDI`) over USB.
2. It starts `debugserver` located inside the developer image.
3. It tunnels the communication port back to the host via `usbmuxd`.
4. It connects host LLDB, giving developers on Linux and macOS full debugging capabilities.
