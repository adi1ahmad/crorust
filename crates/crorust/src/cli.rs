use crate::cargo::CargoWorkspaceConfig;
use crate::{commands, ProjectBuildArguments, SessionExecutionContext, TargetPlatform};
use anyhow::Result;
use clap::{Parser, Subcommand};

/// croRust: Zero-configuration cross-platform build, packaging, and deployment tool
#[derive(Parser, Debug)]
#[clap(
    name = "crorust",
    author,
    version,
    about = "Cross-platform build, packaging, and deployment tool for Rust applications.",
    long_about = "croRust enables zero-configuration, dependency-free application development and packaging \
across Linux, macOS, Windows, Android, iOS, and Web without proprietary external IDEs or SDK managers.",
    after_help = "EXAMPLES:
  crorust template init my_app             Scaffold a new cross-platform app
  crorust build -p android                 Compile & package APK for Android
  crorust run                              Build and launch on host or connected device
  crorust devices                          List all connected physical and virtual devices
  crorust i                                Audit host toolchains, SDKs, and install missing toolchains
  crorust cache clean                      Purge project caches and packaging artifacts"
)]
pub struct CliArguments {
    #[clap(subcommand)]
    pub selected_command: CliSubcommand,
}

#[derive(Subcommand, Debug)]
pub enum CliSubcommand {
    /// Compile and package application artifacts for target platforms
    #[clap(
        name = "build",
        after_help = "EXAMPLES:
  crorust build -p android                 Build signed Android APK (debug)
  crorust build -p linux --musl --release  Build static standalone Linux AppImage
  crorust build -p windows -a x64          Build 64-bit Windows MSIX package"
    )]
    Build {
        #[clap(flatten)]
        args: ProjectBuildArguments,
    },

    /// Build, install, launch, and stream logs from a device or host desktop (Debug mode)
    #[clap(
        name = "run",
        after_help = "EXAMPLES:
  crorust run                              Run on auto-selected target (device or host)
  crorust run -p android --device adb:123  Run specifically on target Android device
  crorust run -- --arg1 --arg2             Pass extra arguments directly to the app"
    )]
    Run {
        #[clap(flatten)]
        args: ProjectBuildArguments,

        /// Additional launch arguments passed directly to the launched executable
        #[clap(last = true)]
        launch_args: Vec<String>,
    },

    /// Build, package, install, and launch in Release mode with full optimizations
    #[clap(
        name = "deploy",
        after_help = "EXAMPLES:
  crorust deploy -p android                Package optimized APK and launch on device
  crorust deploy -p ios --device imd:...   Deploy signed release IPA to iPhone"
    )]
    Deploy {
        #[clap(flatten)]
        args: ProjectBuildArguments,

        /// Additional launch arguments passed directly to the launched executable
        #[clap(last = true)]
        launch_args: Vec<String>,
    },

    /// Build and attach an interactive LLDB debug session to the target device
    #[clap(
        name = "debug",
        after_help = "EXAMPLES:
  crorust debug -p android                 Launch app on Android with LLDB attached
  crorust debug -p ios                     Attach LLDB to iOS Simulator / device"
    )]
    Debug {
        #[clap(flatten)]
        args: ProjectBuildArguments,
    },

    /// Discover and list connected Android devices, iOS hardware, simulators, and host
    #[clap(
        name = "devices",
        after_help = "EXAMPLES:
  crorust devices                          Display table of all connected targets"
    )]
    Devices,

    /// Audit host compilers, platform SDKs, linkers, sysroots, and install missing toolchains
    #[clap(
        name = "i",
        after_help = "EXAMPLES:
  crorust i                                Perform comprehensive system diagnostics
  crorust i --fix                          Automatically download missing platform SDKs
  crorust i --system                       Use host package manager to install tools"
    )]
    I {
        #[clap(flatten)]
        args: commands::IArgs,
    },

    /// Manage, scaffold, test, and typecheck project templates
    #[clap(
        name = "template",
        after_help = "EXAMPLES:
  crorust template init my_app             Scaffold a new project from reference template
  crorust template info                    Display metadata about the embedded template
  crorust template check                   Typecheck template across desktop, Android & Web
  crorust template run                     Run in-tree template in development mode"
    )]
    Template {
        #[clap(subcommand)]
        action: TemplateCommands,
    },

    /// Manage cryptographic signing certificates, API keys, and identities
    #[clap(
        name = "cert",
        after_help = "EXAMPLES:
  crorust cert inspect identity.pem        Display certificate issuer, expiry, and thumbprint
  crorust cert export-debug --platform android --out debug.pem  Export built-in debug cert"
    )]
    Cert {
        #[clap(subcommand)]
        action: commands::cert::CertSubcommand,
    },

    /// Inspect, prune, and clean hermetic SDK, sysroot, and compiler caches
    #[clap(
        name = "cache",
        after_help = "EXAMPLES:
  crorust cache info                       Show active cache scope and disk usage
  crorust cache prune                      Delete incomplete downloads and stale locks
  crorust cache clean                      Purge all project caches and packaging artifacts
  crorust cache clean --cargo              Purge only Cargo target compilation cache
  crorust cache clean --web                Purge only Web static compilation output
  crorust cache clean --all                Purge project caches and machine-wide global cache
  crorust cache import-xcode Xcode_15.xip  Import Apple SDKs from official Xcode archive"
    )]
    Cache {
        #[clap(subcommand)]
        action: crate::commands::cache::CacheSubcommand,
    },

    /// Generate shell completion scripts for Bash, Zsh, Fish, or PowerShell
    #[clap(
        name = "completions",
        after_help = "EXAMPLES:
  crorust completions bash > ~/.bash_completion.d/crorust
  crorust completions zsh > ~/.zsh/completion/_crorust"
    )]
    Completions {
        /// Target shell to generate completions for
        #[clap(value_enum)]
        shell: clap_complete::Shell,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum TemplateCommands {
    /// Scaffold a new cross-platform application from the embedded reference template
    #[clap(
        name = "init",
        after_help = "EXAMPLES:
  crorust template init my_app             Create 'my_app' project directory
  crorust template init ./games/space_inv  Scaffold project in specific directory"
    )]
    Init {
        /// Name or directory path of the new project
        name: String,
    },

    /// Display metadata, dependencies, and supported platforms of the embedded template
    #[clap(name = "info")]
    Info,

    /// Run the reference template in development mode on connected device or host
    #[clap(name = "run")]
    Run {
        #[clap(flatten)]
        args: ProjectBuildArguments,

        #[clap(last = true)]
        launch_args: Vec<String>,
    },

    /// Compile, package, and launch the reference template in release mode
    #[clap(name = "deploy")]
    Deploy {
        #[clap(flatten)]
        args: ProjectBuildArguments,

        #[clap(last = true)]
        launch_args: Vec<String>,
    },

    /// Compile and package reference template artifacts
    #[clap(name = "build")]
    Build {
        #[clap(flatten)]
        args: ProjectBuildArguments,
    },

    /// Fast typecheck the reference template across host, Android, and Web targets
    #[clap(name = "check")]
    Check {
        /// Platform to verify (android, web, linux, macos, windows, ios)
        #[clap(long)]
        platform: Option<TargetPlatform>,
    },
}

fn partial_build_env() -> Result<()> {
    let config = CargoWorkspaceConfig::locate_workspace_cargo_config(".")?;
    if let Some(config) = &config {
        config.apply_cargo_environment_overrides()?;
    }
    Ok(())
}

fn print_target_banner(env: &SessionExecutionContext) {
    if let Some(device) = env.build_target().target_device() {
        println!(
            "Targeting: {} ({}) [{}]",
            device,
            device
                .device_name()
                .unwrap_or_else(|_| "Unknown Device".into()),
            env.build_target().optimization_profile()
        );
    }
}

impl CliArguments {
    pub fn run(self) -> Result<()> {
        self.selected_command.run()
    }
}

impl CliSubcommand {
    pub fn run(self) -> Result<()> {
        match self {
            Self::Build { args } => {
                let env = SessionExecutionContext::new(args)?;
                commands::build(&env)?;
            }
            Self::Run { args, launch_args } => {
                let env = SessionExecutionContext::new_with_context(args, false)?;
                print_target_banner(&env);
                if env.build_target().target_platform() != TargetPlatform::Web {
                    commands::build(&env)?;
                }
                commands::run(&env, &launch_args)?;
            }
            Self::Deploy { args, launch_args } => {
                let env = SessionExecutionContext::new_with_context(args, true)?;
                print_target_banner(&env);
                if env.build_target().target_platform() != TargetPlatform::Web {
                    commands::build(&env)?;
                }
                commands::run(&env, &launch_args)?;
            }
            Self::Debug { args } => {
                let env = SessionExecutionContext::new(args)?;
                commands::build(&env)?;
                commands::debug(&env)?;
            }
            Self::Devices => {
                partial_build_env()?;
                commands::devices::list_connected_devices()?;
            }
            Self::I { args } => {
                partial_build_env()?;
                commands::run_system_i_check(&args)?;
            }
            Self::Template { action } => match action {
                TemplateCommands::Init { name } => {
                    commands::template::init(&name)?;
                }
                TemplateCommands::Info => {
                    commands::template::info()?;
                }
                TemplateCommands::Run { args, launch_args } => {
                    commands::template::run(args, &launch_args)?;
                }
                TemplateCommands::Deploy { args, launch_args } => {
                    commands::template::deploy(args, &launch_args)?;
                }
                TemplateCommands::Build { args } => {
                    commands::template::build(args)?;
                }
                TemplateCommands::Check { platform } => {
                    commands::template::check(platform)?;
                }
            },
            Self::Cert { action } => {
                commands::cert::run(action)?;
            }
            Self::Cache { action } => {
                commands::cache::run(action)?;
            }
            Self::Completions { shell } => {
                commands::completions::generate(shell);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_template_init_parses() {
        let args = CliArguments::try_parse_from(["crorust", "template", "init", "my_app"])
            .expect("crorust template init must parse successfully");
        match args.selected_command {
            CliSubcommand::Template {
                action: TemplateCommands::Init { name },
            } => {
                assert_eq!(name, "my_app");
            }
            _ => panic!("Expected Template::Init variant"),
        }
    }

    #[test]
    fn test_template_info_parses() {
        let args = CliArguments::try_parse_from(["crorust", "template", "info"])
            .expect("crorust template info must parse successfully");
        match args.selected_command {
            CliSubcommand::Template {
                action: TemplateCommands::Info,
            } => {}
            _ => panic!("Expected Template::Info variant"),
        }
    }

    #[test]
    fn test_devices_command_parses() {
        let args = CliArguments::try_parse_from(["crorust", "devices"])
            .expect("crorust devices must parse successfully");
        assert!(matches!(args.selected_command, CliSubcommand::Devices));
    }

    #[test]
    fn test_i_command_parses() {
        let args = CliArguments::try_parse_from(["crorust", "i"])
            .expect("crorust i must parse successfully");
        assert!(matches!(args.selected_command, CliSubcommand::I { .. }));

        let args_fix = CliArguments::try_parse_from(["crorust", "i", "--fix"])
            .expect("crorust i --fix must parse successfully");
        match args_fix.selected_command {
            CliSubcommand::I { args } => assert!(args.fix),
            _ => panic!("Expected I variant"),
        }
    }

    #[test]
    fn test_cache_clean_command_parses() {
        let args = CliArguments::try_parse_from(["crorust", "cache", "clean", "--all"])
            .expect("crorust cache clean --all must parse successfully");
        match args.selected_command {
            CliSubcommand::Cache {
                action: commands::cache::CacheSubcommand::Clean(args),
            } => {
                assert!(args.all);
            }
            _ => panic!("Expected Cache::Clean variant"),
        }

        let args_selective =
            CliArguments::try_parse_from(["crorust", "cache", "clean", "--cargo", "--web"])
                .expect("crorust cache clean --cargo --web must parse successfully");
        match args_selective.selected_command {
            CliSubcommand::Cache {
                action: commands::cache::CacheSubcommand::Clean(args),
            } => {
                assert!(args.cargo);
                assert!(args.web);
                assert!(!args.tools);
            }
            _ => panic!("Expected Cache::Clean variant"),
        }
    }

    #[test]
    fn test_cert_inspect_parses() {
        let args = CliArguments::try_parse_from(["crorust", "cert", "inspect", "my_cert.pem"])
            .expect("crorust cert inspect must parse successfully");
        match args.selected_command {
            CliSubcommand::Cert {
                action: commands::cert::CertSubcommand::Inspect { file },
            } => {
                assert_eq!(file, PathBuf::from("my_cert.pem"));
            }
            _ => panic!("Expected Cert::Inspect variant"),
        }
    }
}
