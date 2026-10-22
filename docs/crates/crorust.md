# Crate: `crorust` (CLI & Workflow Orchestrator)

## 1. Overview & Architectural Role

`crates/crorust` serves as the user-facing CLI binary and top-level pipeline coordinator. It interprets command-line flags, parses `manifest.toml`, queries device backends, provisions hermetic SDKs, synthesizes LLVM compiler environments, drives Cargo, and invokes platform bundlers.

```mermaid
flowchart TB
    %% Styling Classes (Dark Mode & Dual-Theme High Contrast)
    classDef cli fill:#1f3354,stroke:#388bfd,stroke-width:1.5px,color:#e6edf3
    classDef cmd fill:#3d2e14,stroke:#d29922,stroke-width:1.5px,color:#e6edf3
    classDef driver fill:#163824,stroke:#3fb950,stroke-width:1.5px,color:#e6edf3

    subgraph CLI["CLI Ingestion & Session"]
        Args["Command Line Arguments<br/><i>clap derive</i>"]
        Man["manifest.toml<br/><i>serde strict validation</i>"]
        Ctx["SessionExecutionContext<br/><i>target, cache scope, offline</i>"]
        Args & Man --> Ctx
    end

    subgraph Dispatch["Command Dispatcher"]
        Ctx --> CMD{"Subcommand"}
        CMD -->|build| C_Bld["commands::build"]
        CMD -->|run| C_Run["commands::run"]
        CMD -->|deploy| C_Dep["commands::deploy"]
        CMD -->|debug| C_Dbg["commands::debug"]
        CMD -->|devices| C_Dev["commands::devices"]
        CMD -->|i / info| C_I["commands::i"]
        CMD -->|cache| C_Cch["commands::cache"]
        CMD -->|template| C_Tmp["commands::template"]
    end

    subgraph Drivers["Engine Drivers"]
        C_Bld & C_Run & C_Dep --> ToolMgr["download::ToolManager"]
        C_Bld & C_Run & C_Dep --> Synth["cargo::CargoEnvironmentSynthesizer"]
        C_Bld & C_Run & C_Dep --> Exec["cargo::CargoWorkspaceSession"]
        C_Run & C_Dbg --> DevBridge["devices::TargetDevice"]
    end

    %% Subgraph Styling
    style CLI fill:none,stroke:#388bfd,stroke-width:1.5px,stroke-dasharray:4 4,color:#e6edf3
    style Dispatch fill:none,stroke:#d29922,stroke-width:1.5px,stroke-dasharray:4 4,color:#e6edf3
    style Drivers fill:none,stroke:#3fb950,stroke-width:1.5px,stroke-dasharray:4 4,color:#e6edf3

    %% Class Assignments
    class Args,Man,Ctx cli
    class CMD,C_Bld,C_Run,C_Dep,C_Dbg,C_Dev,C_I,C_Cch,C_Tmp cmd
    class ToolMgr,Synth,Exec,DevBridge driver
```

---

## 2. Key Modules & Subsystems

### 1. `cli.rs` & `commands/`
`crorust` uses `clap` with derive macros to parse user input. Subcommands are cleanly separated into dedicated modules:
- `commands::build`: Coordinates compilation and packaging without running the artifact.
- `commands::run`: Builds, installs to target device, launches the process, and streams stdout/logcat logs.
- `commands::deploy`: Release-mode equivalent of `run`.
- `commands::debug`: Deploys a debug build and opens an interactive LLDB debugging session via port forwarding.
- `commands::devices`: Discovers and displays connected physical and virtual devices.
- `commands::i`: Audits compilers, linkers, and SDKs. With `--fix`, provisions all missing components automatically.
- `commands::cache`: Manages disk usage, cleaning targets, and pruning orphaned locks.
- `commands::template`: Scaffolds new projects from `templates/crorust.ruxt`.

### 2. `core/`
Encapsulates domain models:
- `target.rs`: Defines `SessionBuildTarget`, `TargetPlatform`, `TargetArch`, and `LinuxLibcVariant`.
- `env.rs`: `SessionExecutionContext` containing immutable session settings.
- `contracts.rs`: Defines the foundational interfaces (`ToolchainProvider`, `BuildEnvironmentSynthesizer`, `PackageAssembler`).

### 3. `download/`
Handles hermetic toolchain acquisition:
- `catalog.rs`: Declarative index of verified tool release archives (Adoptium JDK, Android NDK, Kotlin, Windows SDK).
- `manager.rs`: `ToolManager` implementing cross-process locking (`fd-lock`), streaming SHA-256 checks, and atomic extractions.

### 4. `task.rs`
Provides terminal user feedback:
- Wraps `indicatif` to render multi-line progress bars and activity spinners.
- Silences animation in non-interactive CI environments to keep logs clean.
