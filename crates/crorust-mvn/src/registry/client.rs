use crate::model::coordinate::{MavenCoordinates, RemoteArtifactRef, SemverVersion};
use crate::model::dependency::DependencyRequirement;
use crate::model::pom::ProjectObjectModel;
use crate::registry::metadata::MavenMetadataDocument;
use anyhow::{ensure, Context, Result};
use pubgrub::error::PubGrubError;
use pubgrub::range::Range;
use pubgrub::report::{DefaultStringReporter, Reporter};
use pubgrub::solver::{Dependencies, DependencyProvider};
use std::borrow::Borrow;
use std::collections::HashMap;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::RwLock;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RepositoryHttpCredentials {
    Basic { username: String, password: String },
    Bearer { token: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteArtifactRepository {
    pub url: String,
    pub auth: Option<RepositoryHttpCredentials>,
}

impl RemoteArtifactRepository {
    pub fn new(raw_url: impl Into<String>) -> Self {
        let mut trimmed_url = raw_url.into();
        while trimmed_url.ends_with('/') {
            trimmed_url.pop();
        }
        Self {
            url: trimmed_url,
            auth: None,
        }
    }

    pub fn with_auth(raw_url: impl Into<String>, auth: RepositoryHttpCredentials) -> Self {
        let mut trimmed_url = raw_url.into();
        while trimmed_url.ends_with('/') {
            trimmed_url.pop();
        }
        Self {
            url: trimmed_url,
            auth: Some(auth),
        }
    }
}

/// A single download request within a batch download operation.
#[derive(Clone, Debug)]
pub struct DownloadBatchItem {
    pub url: String,
    pub dest: PathBuf,
    pub auth: Option<RepositoryHttpCredentials>,
    pub expected_sha256: Option<String>,
}

impl DownloadBatchItem {
    pub fn new(url: impl Into<String>, dest: impl Into<PathBuf>) -> Self {
        Self {
            url: url.into(),
            dest: dest.into(),
            auth: None,
            expected_sha256: None,
        }
    }

    pub fn with_auth(mut self, auth: Option<RepositoryHttpCredentials>) -> Self {
        self.auth = auth;
        self
    }

    pub fn with_sha256(mut self, sha256: Option<String>) -> Self {
        self.expected_sha256 = sha256;
        self
    }
}

pub trait Download: Send + Sync {
    fn download(&self, url: &str, dest: &Path) -> Result<()>;

    fn download_with_auth(
        &self,
        url: &str,
        dest: &Path,
        _auth: Option<&RepositoryHttpCredentials>,
    ) -> Result<()> {
        self.download(url, dest)
    }

    /// Download a file quietly without displaying individual progress bars.
    /// Ideal for small metadata XMLs and POM descriptors.
    fn download_quiet(
        &self,
        url: &str,
        dest: &Path,
        auth: Option<&RepositoryHttpCredentials>,
    ) -> Result<()> {
        self.download_with_auth(url, dest, auth)
    }

    /// Download multiple files concurrently using an aggregate progress reporter.
    fn download_batch(&self, items: &[DownloadBatchItem]) -> Result<()> {
        for item in items {
            self.download_with_auth(&item.url, &item.dest, item.auth.as_ref())?;
        }
        Ok(())
    }
}

pub const GOOGLE_MAVEN: &str = "https://maven.google.com";

pub const MAVEN_CENTRAL: &str = "https://repo1.maven.org/maven2";

pub const GRADLE_PLUGIN_PORTAL: &str = "https://plugins.gradle.org/m2";

pub struct RepositoryClient<D: Download> {
    fetcher: D,
    cache_directory: PathBuf,
    active_repositories: Vec<RemoteArtifactRepository>,
    registered_urls: Vec<String>,
    pinned_dependencies:
        HashMap<(MavenCoordinates, SemverVersion), Dependencies<MavenCoordinates, SemverVersion>>,
    /// Maps group ID (e.g. "org.jetbrains.kotlin") to the index in active_repositories
    repo_affinity: RwLock<HashMap<String, usize>>,
    /// Maps exact artifact coordinate & version to the index in active_repositories where its POM was found
    artifact_repo: RwLock<HashMap<(MavenCoordinates, SemverVersion), usize>>,
}

impl<D: Download> RepositoryClient<D> {
    pub fn new(cache_dir: PathBuf, fetcher: D) -> Result<Self> {
        std::fs::create_dir_all(&cache_dir).with_context(|| {
            format!(
                "Failed creating Maven cache directory: {}",
                cache_dir.display()
            )
        })?;
        Ok(Self {
            cache_directory: cache_dir,
            fetcher,
            active_repositories: Vec::new(),
            registered_urls: Vec::new(),
            pinned_dependencies: HashMap::new(),
            repo_affinity: RwLock::new(HashMap::new()),
            artifact_repo: RwLock::new(HashMap::new()),
        })
    }

    /// Determines the optimal repository search order for a given group ID.
    pub fn prioritized_repositories(&self, group: &str) -> Vec<(usize, &RemoteArtifactRepository)> {
        // 1. Check dynamic affinity cache
        if let Ok(guard) = self.repo_affinity.read() {
            if let Some(&preferred_idx) = guard.get(group) {
                if preferred_idx < self.active_repositories.len() {
                    let mut order = Vec::with_capacity(self.active_repositories.len());
                    order.push((preferred_idx, &self.active_repositories[preferred_idx]));
                    for (i, repo) in self.active_repositories.iter().enumerate() {
                        if i != preferred_idx {
                            order.push((i, repo));
                        }
                    }
                    return order;
                }
            }
        }

        // 2. Check static well-known prefixes
        let prefers_central = group.starts_with("org.jetbrains")
            || group.starts_with("org.apache")
            || group.starts_with("commons-")
            || group.starts_with("org.jspecify")
            || group.starts_with("com.google.guava");

        let prefers_google = group.starts_with("androidx.")
            || group.starts_with("com.android.")
            || group.starts_with("com.google.android.")
            || group.starts_with("android.arch.");

        let mut order: Vec<(usize, &RemoteArtifactRepository)> =
            self.active_repositories.iter().enumerate().collect();

        if prefers_central {
            order.sort_by_key(|(_, repo)| !repo.url.contains("repo1.maven.org"));
        } else if prefers_google {
            order.sort_by_key(|(_, repo)| !repo.url.contains("maven.google.com"));
        }

        order
    }

    /// Records that an artifact group was successfully resolved from a specific repository.
    pub fn record_affinity(&self, group: &str, repo_idx: usize) {
        if let Ok(mut guard) = self.repo_affinity.write() {
            guard.insert(group.to_string(), repo_idx);
        }
    }

    /// Records the exact repository where an artifact's POM descriptor was discovered.
    pub fn record_artifact_repository(
        &self,
        package: &MavenCoordinates,
        version: &SemverVersion,
        repo_idx: usize,
    ) {
        if let Ok(mut guard) = self.artifact_repo.write() {
            guard.insert((package.clone(), version.clone()), repo_idx);
        }
    }

    /// Retrieves the recorded repository index for a specific package version, if known.
    pub fn get_artifact_repository(
        &self,
        package: &MavenCoordinates,
        version: &SemverVersion,
    ) -> Option<usize> {
        self.artifact_repo
            .read()
            .ok()
            .and_then(|guard| guard.get(&(package.clone(), version.clone())).copied())
    }

    pub fn add_repository(&mut self, endpoint: impl Into<String>) {
        let repo = RemoteArtifactRepository::new(endpoint);
        if !repo.url.is_empty() && !self.registered_urls.iter().any(|u| u == &repo.url) {
            self.registered_urls.push(repo.url.clone());
            self.active_repositories.push(repo);
        }
    }

    pub fn add_repository_with_auth(
        &mut self,
        endpoint: impl Into<String>,
        credentials: RepositoryHttpCredentials,
    ) {
        let repo = RemoteArtifactRepository::with_auth(endpoint, credentials);
        if !repo.url.is_empty() && !self.registered_urls.iter().any(|u| u == &repo.url) {
            self.registered_urls.push(repo.url.clone());
            self.active_repositories.push(repo);
        }
    }

    pub fn add_repositories<I, S>(&mut self, endpoints: I)
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        for ep in endpoints {
            self.add_repository(ep);
        }
    }

    pub fn add_default_repositories(&mut self) {
        self.add_repository(GOOGLE_MAVEN);
        self.add_repository(MAVEN_CENTRAL);
    }

    pub fn repositories(&self) -> &[String] {
        &self.registered_urls
    }

    pub fn add_package(
        &mut self,
        package: MavenCoordinates,
        version: SemverVersion,
        requirements: Vec<DependencyRequirement>,
    ) -> Result<()> {
        let mut mapped_deps = Vec::with_capacity(requirements.len());
        for req in requirements {
            mapped_deps.push((req.package(), req.range()?));
        }
        self.pinned_dependencies.insert(
            (package, version),
            Dependencies::Known(mapped_deps.into_iter().collect()),
        );
        Ok(())
    }

    pub fn resolve(
        &self,
        root_coordinate: MavenCoordinates,
        target_version: SemverVersion,
    ) -> Result<Vec<PathBuf>> {
        let solution =
            pubgrub::solver::resolve(self, root_coordinate, target_version).map_err(|err| {
                if let PubGrubError::NoSolution(mut derivation_tree) = err {
                    derivation_tree.collapse_no_versions();
                    anyhow::anyhow!("{}", DefaultStringReporter::report(&derivation_tree))
                } else {
                    anyhow::anyhow!("PubGrub resolution error: {:?}", err)
                }
            })?;

        // 1. Classify packages into already-cached vs needing download
        let mut download_items = Vec::new();
        let mut resolved_paths = Vec::with_capacity(solution.len());
        let mut aars_needing_unpack = Vec::new();

        for (coord, ver) in &solution {
            if self
                .pinned_dependencies
                .contains_key(&(coord.clone(), ver.clone()))
            {
                continue;
            }

            let artifact_ref = RemoteArtifactRef {
                package: coord,
                version: ver,
            };
            let pom_doc = self.pom(artifact_ref)?;
            let packaging = pom_doc.packaging();
            if packaging == "pom" {
                continue;
            }

            let local_path = self.cache_directory.join(artifact_ref.file_name(packaging));

            if !local_path.exists() {
                // Determine repository URL and credentials using exact pairing or prioritized affinity
                let (target_url, auth) =
                    if let Some(repo_idx) = self.get_artifact_repository(coord, ver) {
                        if repo_idx < self.active_repositories.len() {
                            let repo = &self.active_repositories[repo_idx];
                            (artifact_ref.url(&repo.url, packaging), repo.auth.clone())
                        } else if !self.active_repositories.is_empty() {
                            let repos = self.prioritized_repositories(coord.group_id());
                            let repo = repos[0].1;
                            (artifact_ref.url(&repo.url, packaging), repo.auth.clone())
                        } else {
                            anyhow::bail!(
                                "No Maven repositories available to download {}",
                                artifact_ref
                            );
                        }
                    } else if !self.active_repositories.is_empty() {
                        let repos = self.prioritized_repositories(coord.group_id());
                        let repo = repos[0].1;
                        (artifact_ref.url(&repo.url, packaging), repo.auth.clone())
                    } else {
                        anyhow::bail!(
                            "No Maven repositories available to download {}",
                            artifact_ref
                        );
                    };

                download_items
                    .push(DownloadBatchItem::new(target_url, local_path.clone()).with_auth(auth));
            }

            if packaging == "aar" {
                let unpacked_jar = self.cache_directory.join(artifact_ref.file_name("jar"));
                if !unpacked_jar.exists() {
                    aars_needing_unpack.push((local_path.clone(), unpacked_jar));
                }
            }

            resolved_paths.push(local_path);
        }

        // 2. Perform concurrent bounded batch download for missing artifacts
        if !download_items.is_empty() {
            log::info!(
                "Downloading {} missing Maven artifacts in parallel...",
                download_items.len()
            );
            if let Err(batch_err) = self.fetcher.download_batch(&download_items) {
                log::warn!(
                    "Batch download encountered an issue, falling back to individual artifact resolution: {batch_err}"
                );
                for (coord, ver) in &solution {
                    let artifact_ref = RemoteArtifactRef {
                        package: coord,
                        version: ver,
                    };
                    let pom_doc = self.pom(artifact_ref)?;
                    let packaging = pom_doc.packaging();
                    if packaging != "pom" {
                        self.artifact(artifact_ref, packaging)?;
                    }
                }
            }
        }

        // 3. Parallel extract classes.jar from newly downloaded AARs
        if !aars_needing_unpack.is_empty() {
            use rayon::prelude::*;
            aars_needing_unpack.par_iter().try_for_each(
                |(aar_path, target_jar)| -> Result<()> {
                    if let Ok(jar_bytes) =
                        crorust_common::unpack_single_entry_secure(aar_path, "classes.jar")
                    {
                        std::fs::write(target_jar, jar_bytes)?;
                    }
                    Ok(())
                },
            )?;
        }

        log::info!(
            "Resolved {} dependencies successfully",
            resolved_paths.len()
        );
        Ok(resolved_paths)
    }

    pub fn package(&self, package: &MavenCoordinates, version: &SemverVersion) -> Result<PathBuf> {
        let artifact = RemoteArtifactRef { package, version };
        let pom = self.pom(artifact)?;
        self.artifact(artifact, pom.packaging())
    }

    fn versions(
        &self,
        package: &MavenCoordinates,
        constraint_range: &Range<SemverVersion>,
    ) -> Vec<SemverVersion> {
        match self.metadata(package) {
            Ok(metadata) => {
                let all_candidates: Vec<SemverVersion> = metadata
                    .versions()
                    .iter()
                    .filter_map(|v| SemverVersion::from_str(v).ok())
                    .filter(|v| constraint_range.contains(v))
                    .rev()
                    .collect();

                let stable_candidates: Vec<SemverVersion> = all_candidates
                    .iter()
                    .filter(|v| v.is_stable_release())
                    .cloned()
                    .collect();

                if !stable_candidates.is_empty() {
                    stable_candidates
                } else {
                    all_candidates
                }
            }
            Err(err) => {
                log::debug!("Failed retrieving metadata for {}: {err}", package);
                constraint_range.lowest_version().into_iter().collect()
            }
        }
    }

    fn metadata(&self, package: &MavenCoordinates) -> Result<MavenMetadataDocument> {
        let cached_path = self.cache_directory.join(package.file_name());
        if !cached_path.exists() {
            let mut fetch_success = false;
            let repos = self.prioritized_repositories(package.group_id());

            for (idx, repo) in repos {
                let target_url = package.url(&repo.url);
                if self
                    .fetcher
                    .download_quiet(&target_url, &cached_path, repo.auth.as_ref())
                    .is_ok()
                {
                    self.record_affinity(package.group_id(), idx);
                    fetch_success = true;
                    break;
                }
            }
            ensure!(
                fetch_success,
                "Could not fetch maven-metadata.xml for {package}"
            );
        }

        let xml_raw = std::fs::read_to_string(&cached_path).with_context(|| {
            format!("Failed reading cached metadata: {}", cached_path.display())
        })?;
        let doc: MavenMetadataDocument = quick_xml::de::from_str(&xml_raw)
            .with_context(|| format!("Failed parsing metadata XML for {package}"))?;
        Ok(doc)
    }

    fn pom(&self, artifact: RemoteArtifactRef) -> Result<ProjectObjectModel> {
        match self.artifact(artifact, "pom") {
            Ok(pom_path) => {
                let content = std::fs::read_to_string(&pom_path)?;
                let pom: ProjectObjectModel = quick_xml::de::from_str(&content)
                    .with_context(|| format!("Failed parsing POM from {}", pom_path.display()))?;
                Ok(pom)
            }
            Err(err) => {
                log::debug!("Could not fetch POM for {artifact}: {err}");
                Ok(Default::default())
            }
        }
    }

    fn artifact(&self, artifact: RemoteArtifactRef, packaging_ext: &str) -> Result<PathBuf> {
        let local_path = self.cache_directory.join(artifact.file_name(packaging_ext));
        if !local_path.exists() {
            log::info!("Fetching remote artifact {} ({})", artifact, packaging_ext);
            let mut retrieved = false;

            let preferred_order = if let Some(exact_idx) =
                self.get_artifact_repository(artifact.package, artifact.version)
            {
                if exact_idx < self.active_repositories.len() {
                    let mut order = vec![(exact_idx, &self.active_repositories[exact_idx])];
                    for (i, repo) in self.active_repositories.iter().enumerate() {
                        if i != exact_idx {
                            order.push((i, repo));
                        }
                    }
                    order
                } else {
                    self.prioritized_repositories(artifact.package.group_id())
                }
            } else {
                self.prioritized_repositories(artifact.package.group_id())
            };

            for (idx, repo) in preferred_order {
                let target_url = artifact.url(&repo.url, packaging_ext);
                let download_res = if packaging_ext == "pom" {
                    self.fetcher
                        .download_quiet(&target_url, &local_path, repo.auth.as_ref())
                } else {
                    self.fetcher
                        .download_with_auth(&target_url, &local_path, repo.auth.as_ref())
                };

                if download_res.is_ok() {
                    self.record_affinity(artifact.package.group_id(), idx);
                    self.record_artifact_repository(artifact.package, artifact.version, idx);
                    retrieved = true;
                    break;
                }
            }
            ensure!(
                retrieved,
                "Artifact not found across repositories: {artifact} ({packaging_ext})"
            );
        }

        if packaging_ext == "aar" {
            let unpacked_jar = self.cache_directory.join(artifact.file_name("jar"));
            if !unpacked_jar.exists() {
                match crorust_common::unpack_single_entry_secure(&local_path, "classes.jar") {
                    Ok(jar_bytes) => {
                        std::fs::write(&unpacked_jar, jar_bytes)?;
                    }
                    Err(err) => {
                        log::debug!(
                            "classes.jar not present in AAR {artifact} (likely resource-only): {err}"
                        );
                    }
                }
            }
        }

        Ok(local_path)
    }
}

impl<D: Download> DependencyProvider<MavenCoordinates, SemverVersion> for RepositoryClient<D> {
    fn choose_package_version<T: Borrow<MavenCoordinates>, U: Borrow<Range<SemverVersion>>>(
        &self,
        potential_packages: impl Iterator<Item = (T, U)>,
    ) -> Result<(T, Option<SemverVersion>), Box<dyn Error>> {
        let collected: Vec<(T, U)> = potential_packages.collect();

        // Concurrently prefetch any missing metadata descriptors for candidates
        let missing_candidates: Vec<&MavenCoordinates> = collected
            .iter()
            .map(|(pkg, _)| pkg.borrow())
            .filter(|pkg| !self.cache_directory.join(pkg.file_name()).exists())
            .collect();

        if missing_candidates.len() > 1 {
            use rayon::prelude::*;
            missing_candidates.par_iter().for_each(|pkg| {
                let _ = self.metadata(pkg);
            });
        }

        let mut best_selection: Option<(_, Vec<_>)> = None;
        for (pkg, range) in collected {
            let available = self.versions(pkg.borrow(), range.borrow());
            if let Some((_, ref prior)) = best_selection {
                if prior.len() < available.len() {
                    continue;
                }
            }
            let can_short_circuit = available.len() < 2;
            best_selection = Some((pkg, available));
            if can_short_circuit {
                break;
            }
        }

        let (chosen_pkg, version_list) = best_selection
            .ok_or_else(|| anyhow::anyhow!("No packages provided in candidate iteration"))?;
        let primary_version = version_list.into_iter().next();
        Ok((chosen_pkg, primary_version))
    }

    fn get_dependencies(
        &self,
        package: &MavenCoordinates,
        version: &SemverVersion,
    ) -> Result<Dependencies<MavenCoordinates, SemverVersion>, Box<dyn Error>> {
        if let Some(pinned) = self
            .pinned_dependencies
            .get(&(package.clone(), version.clone()))
        {
            return Ok(pinned.clone());
        }

        let pom = match self.pom(RemoteArtifactRef { package, version }) {
            Ok(p) => p,
            Err(err) => {
                log::warn!("Failed loading POM descriptor for {package} {version}: {err}");
                return Ok(Dependencies::Known(Default::default()));
            }
        };

        let deps = pom
            .dependencies()
            .into_iter()
            .filter(|dep| dep.scope() != Some("test"))
            .filter_map(|dep| match dep.range() {
                Ok(r) => Some((dep.package(), r)),
                Err(err) => {
                    log::warn!(
                        "Skipping dependency {:?} with invalid range: {err}",
                        dep.package()
                    );
                    None
                }
            })
            .collect();

        Ok(Dependencies::Known(deps))
    }
}
