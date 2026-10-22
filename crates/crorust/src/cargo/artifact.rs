use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum CompilationArtifact {
    PackageRoot(String),

    ExampleTarget(String),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CompilationCrateKind {
    Bin,

    Lib,

    Staticlib,

    Cdylib,
}

impl CompilationArtifact {
    #[inline]
    pub fn artifact_name(&self) -> &str {
        match self {
            Self::PackageRoot(ident) | Self::ExampleTarget(ident) => ident.as_str(),
        }
    }

    pub fn relative_output_subdir(&self) -> &'static Path {
        match self {
            Self::PackageRoot(_) => Path::new(""),
            Self::ExampleTarget(_) => Path::new("examples"),
        }
    }

    pub fn target_file_name(
        &self,
        crate_kind: CompilationCrateKind,
        target_triple: &str,
    ) -> String {
        let base_name = self.artifact_name();
        let normalized = base_name.replace('-', "_");

        match crate_kind {
            CompilationCrateKind::Bin => {
                if target_triple.contains("windows") {
                    format!("{base_name}.exe")
                } else if target_triple.contains("wasm") {
                    format!("{base_name}.wasm")
                } else {
                    base_name.to_string()
                }
            }
            CompilationCrateKind::Cdylib => {
                if target_triple.contains("windows") {
                    format!("{normalized}.dll")
                } else if target_triple.contains("darwin") || target_triple.contains("ios") {
                    format!("lib{normalized}.dylib")
                } else {
                    format!("lib{normalized}.so")
                }
            }
            CompilationCrateKind::Staticlib => format!("lib{normalized}.a"),
            CompilationCrateKind::Lib => format!("lib{normalized}.rlib"),
        }
    }

    pub fn relative_path(&self, crate_kind: CompilationCrateKind, target_triple: &str) -> PathBuf {
        let sub = self.relative_output_subdir();
        let fname = self.target_file_name(crate_kind, target_triple);
        if sub.as_os_str().is_empty() {
            PathBuf::from(fname)
        } else {
            sub.join(fname)
        }
    }
}

impl AsRef<Path> for CompilationArtifact {
    fn as_ref(&self) -> &Path {
        self.relative_output_subdir()
    }
}
