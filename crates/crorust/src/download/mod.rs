use crate::{task, SessionExecutionContext, TargetArch, TargetPlatform};
use anyhow::{Context, Result};
use crorust_mvn::Download;
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use reqwest::blocking::Client;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{BufWriter, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

pub const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
pub const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(60);

static PART_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Generates a process-unique, thread-unique temporary part file path.
pub fn generate_unique_part_path(dest_file: &Path) -> PathBuf {
    let pid = std::process::id();
    let counter = PART_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let ext = dest_file
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("bin");
    dest_file.with_extension(format!("{ext}.part-{pid}-{counter}"))
}

pub fn create_hardened_http_client() -> Result<Client> {
    Client::builder()
        .connect_timeout(DEFAULT_CONNECT_TIMEOUT)
        .timeout(DEFAULT_READ_TIMEOUT)
        .tcp_keepalive(Some(Duration::from_secs(30)))
        .pool_idle_timeout(Some(Duration::from_secs(90)))
        .build()
        .context("Failed building hardened HTTP client")
}

pub mod apple_catalog;
pub mod apple_discovery;
pub mod apple_pkg;
pub mod catalog;
pub mod dynamic_platform;
pub mod env;
pub mod manager;
pub mod xwin;
pub use manager::ToolManager;

pub struct VerifiableDownload<'a> {
    pub url: &'a str,
    pub expected_sha256: Option<&'a str>,
    pub quiet: bool,
    pub on_chunk_read: Option<&'a (dyn Fn(usize) + Send + Sync)>,
}

impl<'a> VerifiableDownload<'a> {
    pub fn new(url: &'a str, expected_sha256: Option<&'a str>) -> Self {
        Self {
            url,
            expected_sha256,
            quiet: false,
            on_chunk_read: None,
        }
    }

    pub fn new_quiet(url: &'a str, expected_sha256: Option<&'a str>) -> Self {
        Self {
            url,
            expected_sha256,
            quiet: true,
            on_chunk_read: None,
        }
    }

    pub fn with_chunk_callback(mut self, callback: &'a (dyn Fn(usize) + Send + Sync)) -> Self {
        self.on_chunk_read = Some(callback);
        self
    }

    pub fn download(&self, client: &reqwest::blocking::Client, dest_file: &Path) -> Result<()> {
        self.download_with_auth(client, dest_file, None)
    }

    pub fn download_with_auth(
        &self,
        client: &reqwest::blocking::Client,
        dest_file: &Path,
        auth: Option<&crorust_mvn::RepositoryHttpCredentials>,
    ) -> Result<()> {
        let parent = dest_file.parent().context("dest_file has no parent dir")?;
        std::fs::create_dir_all(parent)?;

        let is_interactive = !self.quiet
            && std::io::stdout().is_terminal()
            && std::env::var("CI").is_err()
            && std::env::var("GITHUB_ACTIONS").is_err();

        let draw_target = if is_interactive {
            ProgressDrawTarget::stdout()
        } else {
            ProgressDrawTarget::hidden()
        };

        let pb = ProgressBar::with_draw_target(Some(0), draw_target)
            .with_style(
                ProgressStyle::default_bar()
                    .template("{spinner:.cyan} {prefix:.bold} [{elapsed_precise}] [{bar:30.cyan/blue}] {bytes}/{total_bytes} {msg}")?
                    .progress_chars("━╾─"),
            );
        let file_name = dest_file.file_name().unwrap().to_str().unwrap().to_string();
        pb.set_prefix(file_name);
        pb.set_message("fetching asset");

        let mut req = client.get(self.url);
        if let Some(auth) = auth {
            match auth {
                crorust_mvn::RepositoryHttpCredentials::Basic { username, password } => {
                    req = req.basic_auth(username, Some(password));
                }
                crorust_mvn::RepositoryHttpCredentials::Bearer { token } => {
                    req = req.bearer_auth(token);
                }
            }
        }

        let mut resp = req.send()?;
        anyhow::ensure!(
            resp.status().is_success(),
            "HTTP request to {} failed with status code {}",
            self.url,
            resp.status()
        );

        if let Some(len) = resp.content_length() {
            pb.set_length(len);
        }

        let part_file = generate_unique_part_path(dest_file);

        let mut writer = BufWriter::new(File::create(&part_file)?);
        let mut sha256_hasher = Sha256::new();
        let mut sha1_hasher = sha1::Sha1::new();
        let mut buffer = [0u8; 64 * 1024];

        let stream_result: Result<()> = (|| {
            loop {
                let bytes_read = resp.read(&mut buffer)?;
                if bytes_read == 0 {
                    break;
                }
                writer.write_all(&buffer[..bytes_read])?;
                sha256_hasher.update(&buffer[..bytes_read]);
                sha1_hasher.update(&buffer[..bytes_read]);
                pb.inc(bytes_read as u64);
                if let Some(cb) = self.on_chunk_read {
                    cb(bytes_read);
                }
            }
            writer.flush()?;
            Ok(())
        })();

        if let Err(err) = stream_result {
            let _ = std::fs::remove_file(&part_file);
            return Err(err).context(format!("Failed while downloading from {}", self.url));
        }

        drop(writer);

        if let Some(expected) = self.expected_sha256 {
            let (is_match, actual_hash, algo_name) = if expected.len() == 40 {
                let actual = hex::encode(sha1_hasher.finalize());
                let m = actual.eq_ignore_ascii_case(expected);
                (m, actual, "SHA-1")
            } else {
                let actual = hex::encode(sha256_hasher.finalize());
                let m = actual.eq_ignore_ascii_case(expected);
                (m, actual, "SHA-256")
            };

            if !is_match {
                let _ = std::fs::remove_file(&part_file);
                anyhow::bail!(
                    "{} checksum mismatch for {}\nExpected: {}\nActual:   {}",
                    algo_name,
                    self.url,
                    expected,
                    actual_hash
                );
            }
        }

        if let Err(err) = std::fs::rename(&part_file, dest_file) {
            if err.kind() == std::io::ErrorKind::CrossesDevices {
                std::fs::copy(&part_file, dest_file)?;
                let _ = std::fs::remove_file(&part_file);
            } else {
                let _ = std::fs::remove_file(&part_file);
                return Err(err.into());
            }
        }

        if !self.quiet {
            pb.finish_with_message("📥 downloaded & verified");
        } else {
            pb.finish_and_clear();
        }
        Ok(())
    }
}

pub fn extract_atomic<F>(
    archive_path: &Path,
    dest_dir: &Path,
    lock_dir: &Path,
    tool_name: &str,
    extractor: F,
) -> Result<()>
where
    F: FnOnce(&Path, &Path) -> Result<()>,
{
    let complete_marker = dest_dir.join(".complete");

    if dest_dir.exists() && complete_marker.exists() {
        return Ok(());
    }

    std::fs::create_dir_all(lock_dir)?;
    let sanitized_tool = tool_name.replace(['/', '\\', ':'], "-");
    let lock_file_path = lock_dir.join(format!("{sanitized_tool}.lock"));
    let lock_file = File::create(&lock_file_path)
        .context(format!("Failed to create lockfile at {:?}", lock_file_path))?;
    let mut lock = fd_lock::RwLock::new(lock_file);
    let _guard = lock
        .write()
        .map_err(|e| anyhow::anyhow!("Failed to acquire exclusive lock: {}", e))?;

    if dest_dir.exists() && complete_marker.exists() {
        return Ok(());
    }

    let cache_root = lock_dir
        .parent()
        .context("lock_dir has no parent directory")?;
    let staging_root = cache_root.join("staging");
    std::fs::create_dir_all(&staging_root)?;

    let pid = std::process::id();
    let staging_dir = staging_root.join(format!(".staging-{sanitized_tool}-{pid}"));

    if staging_dir.exists() {
        let _ = std::fs::remove_dir_all(&staging_dir);
    }
    std::fs::create_dir_all(&staging_dir)?;

    let extraction_result = extractor(archive_path, &staging_dir);
    if let Err(err) = extraction_result {
        let _ = std::fs::remove_dir_all(&staging_dir);
        return Err(err).context(format!("Failed to extract archive {:?}", archive_path));
    }

    let marker_path = staging_dir.join(".complete");
    std::fs::write(
        &marker_path,
        format!(
            "artifact: {}\narchive: {}\npid: {}\n",
            tool_name,
            archive_path
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or("unknown"),
            pid
        ),
    )?;

    if let Some(dest_parent) = dest_dir.parent() {
        std::fs::create_dir_all(dest_parent)?;
    }
    if dest_dir.exists() {
        let _ = std::fs::remove_dir_all(dest_dir);
    }

    atomic_rename_with_backoff(&staging_dir, dest_dir)?;

    Ok(())
}

#[allow(clippy::never_loop)]
pub fn atomic_rename_with_backoff(staging_dir: &Path, dest_dir: &Path) -> Result<()> {
    #[allow(unused_mut, unused_variables)]
    let mut attempts = 0;
    loop {
        match std::fs::rename(staging_dir, dest_dir) {
            Ok(()) => return Ok(()),
            Err(err) => {
                #[cfg(windows)]
                if err.kind() == std::io::ErrorKind::PermissionDenied && attempts < 3 {
                    attempts += 1;
                    std::thread::sleep(std::time::Duration::from_millis(25 * (1 << attempts)));
                    continue;
                }

                if err.kind() == std::io::ErrorKind::CrossesDevices
                    || (cfg!(windows) && err.kind() == std::io::ErrorKind::PermissionDenied)
                {
                    crorust_common::copy_dir_all(staging_dir, dest_dir)
                        .context("Cross-device copy fallback failed during atomic extraction")?;
                    let _ = std::fs::remove_dir_all(staging_dir);
                    return Ok(());
                }

                let _ = std::fs::remove_dir_all(staging_dir);
                return Err(err).context(format!(
                    "Failed renaming staging dir '{}' to destination '{}'",
                    staging_dir.display(),
                    dest_dir.display()
                ));
            }
        }
    }
}

pub struct SdkToolchainFetcher<'a> {
    client: Client,
    env: &'a SessionExecutionContext,
}

impl Download for SdkToolchainFetcher<'_> {
    fn download(&self, remote_url: &str, local_dest: &Path) -> Result<()> {
        self.download_with_auth(remote_url, local_dest, None)
    }

    fn download_with_auth(
        &self,
        remote_url: &str,
        local_dest: &Path,
        auth: Option<&crorust_mvn::RepositoryHttpCredentials>,
    ) -> Result<()> {
        let verifier = VerifiableDownload::new(remote_url, None);
        verifier.download_with_auth(&self.client, local_dest, auth)
    }

    fn download_quiet(
        &self,
        remote_url: &str,
        local_dest: &Path,
        auth: Option<&crorust_mvn::RepositoryHttpCredentials>,
    ) -> Result<()> {
        let verifier = VerifiableDownload::new_quiet(remote_url, None);
        verifier.download_with_auth(&self.client, local_dest, auth)
    }

    fn download_batch(&self, items: &[crorust_mvn::DownloadBatchItem]) -> Result<()> {
        if items.is_empty() {
            return Ok(());
        }

        let is_interactive = std::io::stdout().is_terminal()
            && std::env::var("CI").is_err()
            && std::env::var("GITHUB_ACTIONS").is_err();

        let draw_target = if is_interactive {
            ProgressDrawTarget::stdout()
        } else {
            ProgressDrawTarget::hidden()
        };

        let pb = ProgressBar::with_draw_target(Some(items.len() as u64), draw_target)
            .with_style(
                ProgressStyle::default_bar()
                    .template("  {spinner:.cyan} {prefix:.bold} [{bar:30.cyan/blue}] {pos}/{len} items ({msg})")?
                    .progress_chars("━╾─"),
            );
        pb.set_prefix("📥 Downloading dependencies");
        pb.enable_steady_tick(Duration::from_millis(80));

        let start_time = std::time::Instant::now();
        let total_bytes_streamed = std::sync::atomic::AtomicU64::new(0);

        let available = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        let max_threads = std::cmp::min(12, available.max(4));
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(max_threads)
            .build()
            .context("Failed initializing bounded download thread pool")?;

        let results: Vec<Result<()>> = pool.install(|| {
            use rayon::prelude::*;
            items
                .par_iter()
                .map(|item| {
                    let chunk_callback = |chunk_size: usize| {
                        let cur = total_bytes_streamed
                            .fetch_add(chunk_size as u64, Ordering::Relaxed)
                            + (chunk_size as u64);
                        let mib = cur as f64 / 1_048_576.0;
                        pb.set_message(format!("{:.1} MiB", mib));
                    };

                    let verifier =
                        VerifiableDownload::new_quiet(&item.url, item.expected_sha256.as_deref())
                            .with_chunk_callback(&chunk_callback);

                    let res =
                        verifier.download_with_auth(&self.client, &item.dest, item.auth.as_ref());
                    pb.inc(1);
                    res
                })
                .collect()
        });

        for res in results {
            res?;
        }

        let elapsed = start_time.elapsed();
        let final_bytes = total_bytes_streamed.load(Ordering::Relaxed) as f64 / 1_048_576.0;
        pb.finish_with_message(format!(
            "({:.1} MiB across {} packages verified in {:.2}s)",
            final_bytes,
            items.len(),
            elapsed.as_secs_f64()
        ));

        Ok(())
    }
}

impl<'a> Download for &'a SdkToolchainFetcher<'a> {
    fn download(&self, remote_url: &str, local_dest: &Path) -> Result<()> {
        (*self).download(remote_url, local_dest)
    }

    fn download_with_auth(
        &self,
        remote_url: &str,
        local_dest: &Path,
        auth: Option<&crorust_mvn::RepositoryHttpCredentials>,
    ) -> Result<()> {
        (*self).download_with_auth(remote_url, local_dest, auth)
    }

    fn download_quiet(
        &self,
        remote_url: &str,
        local_dest: &Path,
        auth: Option<&crorust_mvn::RepositoryHttpCredentials>,
    ) -> Result<()> {
        (*self).download_quiet(remote_url, local_dest, auth)
    }

    fn download_batch(&self, items: &[crorust_mvn::DownloadBatchItem]) -> Result<()> {
        (*self).download_batch(items)
    }
}

impl<'a> SdkToolchainFetcher<'a> {
    pub fn new(env: &'a SessionExecutionContext) -> Result<Self> {
        let client = create_hardened_http_client()?;
        let download_dir = env.cache_directory().join("download");
        std::fs::create_dir_all(download_dir)?;
        Ok(Self { env, client })
    }

    pub(crate) fn session_env(&self) -> &SessionExecutionContext {
        self.env
    }

    fn install_rustup_target(&self, triple: &str) -> Result<()> {
        task::run(Command::new("rustup").args(["target", "add", triple]))
    }

    pub fn prefetch_target_toolchains(&self) -> Result<()> {
        for target_spec in self.session_env().build_target().iter_compile_targets() {
            self.install_rustup_target(target_spec.rust_target_triple()?)?;
        }
        self.prefetch_target_platform_sdks()
    }

    fn prefetch_target_platform_sdks(&self) -> Result<()> {
        let target_platform = self.session_env().build_target().target_platform();
        let host_platform = TargetPlatform::detect_host()?;

        match target_platform {
            TargetPlatform::Linux if host_platform != TargetPlatform::Linux => {
                if self.session_env().build_target().target_linux_libc()
                    != Some(crate::LinuxLibcVariant::Musl)
                {
                    self.fetch_linux_sysroot()?;
                }
            }
            TargetPlatform::Windows if host_platform != TargetPlatform::Windows => {
                self.fetch_windows_sdk()?;
            }
            TargetPlatform::Macos if host_platform != TargetPlatform::Macos => {
                self.fetch_macos_sdk()?;
            }
            TargetPlatform::Android => {
                self.fetch_android_ndk()?;
                self.fetch_android_jar()?;
                if !self
                    .session_env()
                    .build_target()
                    .is_android_gradle_enabled()
                {
                    self.fetch_android_standalone_compilers()?;
                }
            }
            TargetPlatform::Ios => {
                if self.session_env().build_target().target_apple_env()
                    == Some(crate::AppleTargetEnv::Simulator)
                {
                    self.fetch_ios_simulator_sdk()?;
                } else {
                    self.fetch_ios_sdk()?;
                }
                if let Some(dev) = self.session_env().build_target().target_device() {
                    if let Ok((maj, min)) = dev.target_ios_product_version() {
                        self.fetch_developer_disk_image(maj, min)?;
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn is_sdk_satisfied(&self, output: &Path) -> bool {
        if !output.exists() {
            return false;
        }
        if !output.starts_with(self.session_env().cache_directory()) {
            return true;
        }
        output.join(".complete").is_file()
    }

    pub fn is_sdk_valid_with_hash(&self, output: &Path, expected_sha256: &str) -> bool {
        let marker = output.join(".complete");
        if let Ok(content) = std::fs::read_to_string(marker) {
            content.contains(expected_sha256)
                || content.trim().eq_ignore_ascii_case(expected_sha256)
        } else {
            false
        }
    }

    pub fn fetch_android_jar(&self) -> Result<()> {
        let target_jar = self.env.resolve_android_jar();
        if target_jar.exists() {
            return Ok(());
        }

        let target_api_level = self.env.android_compile_sdk_version();
        let dest_dir = self
            .env
            .resolve_android_sdk()
            .join("platforms")
            .join(format!("android-{target_api_level}"));
        let tool_mgr = ToolManager::new(self.env)?;

        // Tier 1: Check Manifest Override (Option A)
        if let Some(custom) = self
            .env
            .manifest_config()
            .toolchain()
            .get_platform_override(target_api_level)
        {
            eprintln!(
                "\x1b[33m⚠️  [crorust] Supply Chain Notice: Manifest override active for Android platform {} ({}).\x1b[0m",
                target_api_level, custom.url
            );
            let sha = custom.sha256.as_deref().unwrap_or("");
            let artifact = catalog::ToolArtifact {
                category: catalog::ToolCategory::PlatformTools,
                name: "platforms/android-custom",
                version: Box::leak(target_api_level.to_string().into_boxed_str()),
                url: Box::leak(custom.url.clone().into_boxed_str()),
                sha256: Box::leak(sha.to_string().into_boxed_str()),
                archive_kind: catalog::ArchiveKind::Zip,
                strip_components: 1,
                binary_rel_paths: &["android.jar"],
                extraction_policy: catalog::ExtractionPolicy::default(),
            };
            tool_mgr.ensure_tool_to_dir(&artifact, &dest_dir)?;
            return Ok(());
        }

        // Tier 2: Check Pinned Catalog
        let pinned_artifact = catalog::ToolCatalog::get_android_platform(target_api_level);
        if let Some(artifact) = pinned_artifact {
            match tool_mgr.ensure_tool_to_dir(&artifact, &dest_dir) {
                Ok(_) => return Ok(()),
                Err(err) => {
                    eprintln!(
                        "\x1b[33m⚠️  [crorust] Pinned catalog verification failed for Android platform {}: {:?}\x1b[0m",
                        target_api_level, err
                    );
                }
            }
        }

        // Tier 3: Dynamic Google Repository Manifest Fallback (Option B)
        let repo_client = dynamic_platform::GoogleSdkRepositoryClient::new(
            &self.client,
            self.env.cache_directory(),
            self.env.manifest_config().toolchain().mirror_url.as_deref(),
        );
        let dynamic_res = repo_client.resolve_platform(target_api_level)?;
        let dynamic_artifact = catalog::ToolArtifact {
            category: catalog::ToolCategory::PlatformTools,
            name: "platforms/android-dynamic",
            version: Box::leak(target_api_level.to_string().into_boxed_str()),
            url: Box::leak(dynamic_res.url.into_boxed_str()),
            sha256: Box::leak(dynamic_res.checksum.into_boxed_str()),
            archive_kind: catalog::ArchiveKind::Zip,
            strip_components: 1,
            binary_rel_paths: &["android.jar"],
            extraction_policy: catalog::ExtractionPolicy::default(),
        };

        tool_mgr.ensure_tool_to_dir(&dynamic_artifact, &dest_dir)?;
        Ok(())
    }

    pub fn fetch_android_standalone_compilers(&self) -> Result<()> {
        let tool_mgr = ToolManager::new(self.env)?;
        let mut tools_to_fetch = Vec::new();

        if let Some(r8_artifact) = catalog::ToolCatalog::get_r8("9.5.26") {
            if tool_mgr.find_installed(&r8_artifact).is_none() {
                tools_to_fetch.push(r8_artifact);
            }
        }

        if let Some(desugar_libs) = catalog::ToolCatalog::get_desugar_jdk_libs("2.1.4") {
            if tool_mgr.find_installed(&desugar_libs).is_none() {
                tools_to_fetch.push(desugar_libs);
            }
        }

        if let Some(desugar_cfg) = catalog::ToolCatalog::get_desugar_jdk_libs_config("2.1.4") {
            if tool_mgr.find_installed(&desugar_cfg).is_none() {
                tools_to_fetch.push(desugar_cfg);
            }
        }

        if !tools_to_fetch.is_empty() {
            let refs: Vec<&catalog::ToolArtifact> = tools_to_fetch.iter().collect();
            tool_mgr.ensure_tools_parallel(&refs)?;
        }

        Ok(())
    }

    pub fn fetch_windows_sdk(&self) -> Result<()> {
        let output = self.env.resolve_windows_sdk();
        if self.is_sdk_satisfied(&output) {
            return Ok(());
        }

        if let Some(override_cfg) = self
            .env
            .manifest_config()
            .toolchain()
            .get_windows_sdk_override()
        {
            return self.fetch_custom_sysroot(
                "Windows.sdk",
                self.env
                    .manifest_config()
                    .windows_platform_config()
                    .sdk_version(),
                override_cfg,
            );
        }

        // Automatic upstream acquisition via xwin from official Microsoft CDN
        let xwin_mgr = xwin::XWinManager::new(self.env);
        xwin_mgr.ensure_sysroot()?;

        Ok(())
    }

    pub fn fetch_macos_sdk(&self) -> Result<()> {
        let output = self.env.resolve_macos_sdk();
        if self.is_sdk_satisfied(&output) {
            return Ok(());
        }

        if let Some(override_cfg) = self
            .env
            .manifest_config()
            .toolchain()
            .get_macos_sdk_override()
        {
            return self.fetch_custom_sysroot(
                "MacOSX.sdk",
                self.env
                    .manifest_config()
                    .macos_platform_config()
                    .sdk_version
                    .as_deref()
                    .unwrap_or("14.0"),
                override_cfg,
            );
        }

        if self.env.manifest_config().toolchain().is_offline_enforced() {
            anyhow::bail!(
                "macOS SDK sysroot is missing at `{}` and offline mode is enabled.",
                output.display()
            );
        }

        println!("==> Resolving macOS SDK package from Apple Public Software Update CDN...");
        let package_url = apple_catalog::AppleCdnResolver::resolve_clt_sdk_package_url()?;
        let pkg_filename = package_url
            .rsplit_once('/')
            .map(|(_, f)| f)
            .unwrap_or("CLTools_macOS_SDK.pkg");
        let download_dir = self.env.cache_directory().join("download");
        std::fs::create_dir_all(&download_dir)?;
        let pkg_path = download_dir.join(pkg_filename);

        if !pkg_path.exists() {
            println!("==> Downloading official Apple Command Line Tools package from Apple CDN...");
            let verifier = VerifiableDownload::new(&package_url, None);
            let client = create_hardened_http_client()?;
            verifier.download(&client, &pkg_path)?;
        }

        println!("==> Extracting macOS SDK into `{}`...", output.display());
        apple_pkg::ApplePkgExtractor::extract_pkg(&pkg_path, &output)?;
        std::fs::write(output.join(".complete"), b"ok\n")?;

        println!("==> macOS SDK ready at `{}`", output.display());
        Ok(())
    }

    pub fn fetch_android_ndk(&self) -> Result<()> {
        let output = self.env.resolve_android_ndk();
        if self.is_sdk_satisfied(&output) {
            return Ok(());
        }

        if let Some(override_cfg) = self
            .env
            .manifest_config()
            .toolchain()
            .get_android_ndk_override()
        {
            return self.fetch_custom_ndk(override_cfg);
        }

        let artifact = catalog::ToolCatalog::get_ndk();
        let tool_mgr = ToolManager::new(self.env)?;
        tool_mgr.ensure_tool(&artifact)?;
        Ok(())
    }

    pub fn fetch_ios_sdk(&self) -> Result<()> {
        let output = self.env.resolve_ios_sdk();
        if self.is_sdk_satisfied(&output) {
            return Ok(());
        }

        // Tier 1: Check host Xcode & Apple SDK discovery
        if let Some(host_sdk) = apple_discovery::AppleSdkDiscovery::locate_ios_device_sdk() {
            println!("==> Found host Apple iOS SDK at `{}`", host_sdk.display());
            if let Some(parent) = output.parent() {
                std::fs::create_dir_all(parent)?;
            }
            crorust_common::copy_dir_all(&host_sdk, &output)?;
            std::fs::write(output.join(".complete"), b"ok\n")?;
            return Ok(());
        }

        // Tier 2: Check manifest custom override
        if let Some(override_cfg) = self
            .env
            .manifest_config()
            .toolchain()
            .get_ios_sdk_override()
        {
            return self.fetch_custom_sysroot(
                "iPhoneOS.sdk",
                self.env
                    .manifest_config()
                    .ios_platform_config()
                    .sdk_version
                    .as_deref()
                    .unwrap_or("16.5"),
                override_cfg,
            );
        }

        if self.env.manifest_config().toolchain().is_offline_enforced() {
            anyhow::bail!(
                "Apple iOS SDK (iPhoneOS.sdk) is missing at `{}` and offline mode is enabled.\n\
                 To build offline, configure `[ios] sdk = \"/path/to/iPhoneOS.sdk\"` in manifest.toml or export CRORUST_IOS_SDK.",
                output.display()
            );
        }

        // Tier 3: Canonical upstream community catalog (theos/sdks)
        let artifact = catalog::ToolCatalog::get_sysroot("iPhoneOS.sdk")
            .context("iPhoneOS.sdk missing from ToolCatalog")?;
        println!(
            "==> Fetching upstream Apple iOS SDK from {}...",
            artifact.url
        );
        let tool_mgr = ToolManager::new(self.env)?;
        tool_mgr.ensure_tool(&artifact)?;
        Ok(())
    }

    fn fetch_custom_sysroot(
        &self,
        name: &str,
        version: &str,
        config: &crate::config::CustomPlatformArtifactConfig,
    ) -> Result<()> {
        let dest_dir = self
            .env
            .cache_directory()
            .join("sysroots")
            .join(name)
            .join(version);
        if dest_dir.join(".complete").exists() {
            return Ok(());
        }

        if self.env.manifest_config().toolchain().is_offline_enforced() {
            anyhow::bail!(
                "Custom sysroot `{}` version `{}` is missing from cache `{}` and offline mode is enabled.",
                name,
                version,
                dest_dir.display()
            );
        }

        let download_dir = self.env.cache_directory().join("download");
        std::fs::create_dir_all(&download_dir)?;
        let filename = config
            .url
            .rsplit_once('/')
            .map(|(_, f)| f)
            .unwrap_or("custom_sysroot.tar.zst");
        let archive_path = download_dir.join(filename);

        if !archive_path.exists() {
            let verifier = VerifiableDownload::new(&config.url, config.sha256.as_deref());
            let client = create_hardened_http_client()?;
            verifier.download(&client, &archive_path)?;
        }

        let lock_dir = self.env.cache_directory().join("locks");
        extract_atomic(
            &archive_path,
            &dest_dir,
            &lock_dir,
            name,
            |archive, staging| {
                manager::extract_any_archive_with_strip(
                    archive,
                    staging,
                    1,
                    catalog::ExtractionPolicy::default(),
                )
            },
        )?;

        Ok(())
    }

    fn fetch_custom_ndk(&self, config: &crate::config::CustomPlatformArtifactConfig) -> Result<()> {
        let version = self
            .env
            .manifest_config()
            .android_platform_config()
            .ndk_version
            .as_deref()
            .unwrap_or("26.3.11579264");
        let dest_dir = self
            .env
            .cache_directory()
            .join("tools")
            .join("ndk")
            .join(version);
        if dest_dir.join(".complete").exists() {
            return Ok(());
        }

        if self.env.manifest_config().toolchain().is_offline_enforced() {
            anyhow::bail!(
                "Custom Android NDK version `{}` is missing from cache `{}` and offline mode is enabled.",
                version,
                dest_dir.display()
            );
        }

        let download_dir = self.env.cache_directory().join("download");
        std::fs::create_dir_all(&download_dir)?;
        let filename = config
            .url
            .rsplit_once('/')
            .map(|(_, f)| f)
            .unwrap_or("custom_ndk.tar.zst");
        let archive_path = download_dir.join(filename);

        if !archive_path.exists() {
            let verifier = VerifiableDownload::new(&config.url, config.sha256.as_deref());
            let client = create_hardened_http_client()?;
            verifier.download(&client, &archive_path)?;
        }

        let lock_dir = self.env.cache_directory().join("locks");
        extract_atomic(
            &archive_path,
            &dest_dir,
            &lock_dir,
            "Android.ndk",
            |archive, staging| {
                manager::extract_any_archive_with_strip(
                    archive,
                    staging,
                    1,
                    catalog::ExtractionPolicy::default(),
                )
            },
        )?;

        Ok(())
    }

    pub fn fetch_ios_simulator_sdk(&self) -> Result<()> {
        let output = self.env.resolve_ios_simulator_sdk();
        if self.is_sdk_satisfied(&output) {
            return Ok(());
        }

        // Tier 1: Check host Xcode & Apple SDK discovery
        if let Some(host_sdk) = apple_discovery::AppleSdkDiscovery::locate_ios_simulator_sdk() {
            println!(
                "==> Found host Apple iOS Simulator SDK at `{}`",
                host_sdk.display()
            );
            if let Some(parent) = output.parent() {
                std::fs::create_dir_all(parent)?;
            }
            crorust_common::copy_dir_all(&host_sdk, &output)?;
            std::fs::write(output.join(".complete"), b"ok\n")?;
            return Ok(());
        }

        // Tier 2: Check manifest custom override
        if let Some(override_cfg) = self
            .env
            .manifest_config()
            .toolchain()
            .get_ios_simulator_sdk_override()
        {
            return self.fetch_custom_sysroot(
                "iPhoneSimulator.sdk",
                self.env
                    .manifest_config()
                    .ios_platform_config()
                    .sdk_version
                    .as_deref()
                    .unwrap_or("16.5"),
                override_cfg,
            );
        }

        // Tier 3: Actionable diagnostic error
        anyhow::bail!(
            "Apple iOS Simulator SDK (iPhoneSimulator.sdk) is not found in cache `{}`.\n\
             Due to Apple licensing constraints, proprietary Apple SDKs are not distributed publicly.\n\
             To build for iOS Simulator on this system, you can:\n\
               1. Configure a local SDK path in manifest.toml:\n\
                    [ios]\n\
                    sdk = \"/path/to/iPhoneSimulator.sdk\"\n\
                  or export CRORUST_IOS_SIMULATOR_SDK=\"/path/to/iPhoneSimulator.sdk\"\n\
               2. Specify an enterprise or community archive mirror in manifest.toml:\n\
                    [toolchain.apple.ios_simulator_sdk]\n\
                    url = \"https://your-mirror.example.com/iPhoneSimulator.sdk.tar.xz\"\n\
                    sha256 = \"...\"\n\
               3. Extract the SDK directly from an official Apple Xcode .xip or .app:\n\
                    crorust cache import-xcode /path/to/Xcode_15.xip",
            output.display()
        );
    }

    pub fn fetch_developer_disk_image(&self, major: u32, minor: u32) -> Result<()> {
        let output = self.env.resolve_developer_disk_image(major, minor);
        if output.exists() {
            return Ok(());
        }
        let target_dir = output
            .parent()
            .context("developer_disk_image has no parent")?;
        let url = format!(
            "https://github.com/mspvirajpatel/Xcode_Developer_Disk_Images/releases/download/{major}.{minor}/{major}.{minor}.zip"
        );
        let tool_mgr = ToolManager::new(self.env)?;
        tool_mgr.ensure_dynamic_zip(
            &format!("DeveloperDiskImage-{major}.{minor}"),
            &url,
            target_dir,
        )
    }

    pub fn fetch_linux_sysroot(&self) -> Result<()> {
        let output = self.env.linux_sysroot();
        if self.is_sdk_satisfied(&output) {
            return Ok(());
        }
        let arch_name = match self
            .session_env()
            .build_target()
            .target_architectures()
            .first()
            .copied()
            .unwrap_or(TargetArch::X64)
        {
            TargetArch::X64 => "Linux.sysroot-x86_64",
            TargetArch::Arm64 => "Linux.sysroot-aarch64",
        };
        let artifact = catalog::ToolCatalog::get_sysroot(arch_name)
            .with_context(|| format!("{arch_name} missing from ToolCatalog"))?;
        let tool_mgr = ToolManager::new(self.env)?;
        tool_mgr.ensure_tool(&artifact)?;
        Ok(())
    }
}
