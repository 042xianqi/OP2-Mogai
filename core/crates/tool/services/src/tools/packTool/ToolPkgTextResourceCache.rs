use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::{Arc, Mutex};

use operit_host_api::FileSystemHost;
use operit_plugin_sdk::toolpkg::{ToolPkgParser::ToolPkgArchiveParser, ToolPkgProtection};

type TextResources = Arc<BTreeMap<String, String>>;
type PackageResources = Arc<Mutex<Option<(String, TextResources)>>>;

/// Shares source snapshots, never JavaScript state, across a package's page sessions.
#[derive(Default)]
pub(super) struct ToolPkgTextResourceCache {
    packages: Mutex<BTreeMap<String, PackageResources>>,
}

impl ToolPkgTextResourceCache {
    /// Serializes only loads of the same package; unrelated packages load independently.
    pub(super) fn get_or_load(
        &self,
        package: &str,
        signature: &str,
        load: impl FnOnce() -> Result<BTreeMap<String, String>, String>,
    ) -> Result<(TextResources, bool), String> {
        let slot = self
            .packages
            .lock()
            .map_err(|error| error.to_string())?
            .entry(package.into())
            .or_default()
            .clone();
        let mut entry = slot.lock().map_err(|error| error.to_string())?;
        if let Some((cached_signature, resources)) = &*entry {
            if cached_signature == signature {
                return Ok((resources.clone(), true));
            }
        }
        let resources = Arc::new(load()?);
        *entry = Some((signature.into(), resources.clone()));
        Ok((resources, false))
    }

    /// Detaches in-flight loads too, so they cannot republish an invalidated snapshot.
    pub(super) fn invalidate(&self, package: &str) {
        self.packages
            .lock()
            .expect("text resource cache mutex poisoned")
            .remove(package);
    }

    pub(super) fn retain_packages(&self, packages: &BTreeSet<String>) {
        self.packages
            .lock()
            .expect("text resource cache mutex poisoned")
            .retain(|name, _| packages.contains(name));
    }

    pub(super) fn clear(&self) {
        self.packages
            .lock()
            .expect("text resource cache mutex poisoned")
            .clear();
    }
}

/// Reads from one validated cache directory without reentering package cache validation.
pub(super) fn read_text_resources(
    host: &dyn FileSystemHost,
    directory: &Path,
) -> Result<BTreeMap<String, String>, String> {
    fn collect(
        host: &dyn FileSystemHost,
        root: &Path,
        directory: &Path,
        resources: &mut BTreeMap<String, String>,
    ) -> Result<(), String> {
        for entry in host
            .listFiles(&directory.to_string_lossy())
            .map_err(|error| error.to_string())?
        {
            if entry.name.is_empty()
                || entry.name == "."
                || entry.name == ".."
                || entry.name.contains(['/', '\\'])
            {
                return Err(format!("Invalid ToolPkg resource name: {}", entry.name));
            }
            let path = directory.join(&entry.name);
            if entry.isDirectory {
                collect(host, root, &path, resources)?;
                continue;
            }
            let relative = path
                .strip_prefix(root)
                .map_err(|error| error.to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            if relative == super::TOOLPKG_CACHE_SIGNATURE_FILE {
                continue;
            }
            let normalized = ToolPkgArchiveParser::normalizeResourcePath(&relative)
                .ok_or_else(|| format!("Invalid ToolPkg resource path: {relative}"))?;
            let bytes = host
                .readFileBytes(&path.to_string_lossy())
                .map_err(|error| format!("Cannot read ToolPkg resource {relative}: {error}"))?;
            let bytes = ToolPkgProtection::decryptIfNeeded(&bytes)
                .map_err(|error| format!("Cannot decrypt ToolPkg resource {relative}: {error}"))?;
            if let Ok(text) = String::from_utf8(bytes) {
                resources.insert(normalized.to_ascii_lowercase(), text);
            }
        }
        Ok(())
    }
    let mut resources = BTreeMap::new();
    collect(host, directory, directory, &mut resources)?;
    Ok(resources)
}

#[cfg(test)]
#[path = "tests/ToolPkgTextResourceCacheTests.rs"]
mod tests;
