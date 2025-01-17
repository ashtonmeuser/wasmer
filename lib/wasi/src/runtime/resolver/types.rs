use std::{
    collections::BTreeMap,
    fmt::{Debug, Display},
    ops::Deref,
    path::PathBuf,
    str::FromStr,
};

use anyhow::Context;
use semver::VersionReq;

use crate::{bin_factory::BinaryPackage, http::HttpClient, runtime::resolver::InMemoryCache};

#[async_trait::async_trait]
pub trait PackageResolver: Debug {
    /// Resolve a package, loading all dependencies.
    async fn resolve_package(
        &self,
        pkg: &WebcIdentifier,
        client: &(dyn HttpClient + Send + Sync),
    ) -> Result<BinaryPackage, ResolverError>;

    /// Wrap the [`PackageResolver`] in basic in-memory cache.
    fn with_cache(self) -> InMemoryCache<Self>
    where
        Self: Sized,
    {
        InMemoryCache::new(self)
    }
}

#[async_trait::async_trait]
impl<D, R> PackageResolver for D
where
    D: Deref<Target = R> + Debug + Send + Sync,
    R: PackageResolver + Send + Sync + ?Sized,
{
    /// Resolve a package, loading all dependencies.
    async fn resolve_package(
        &self,
        pkg: &WebcIdentifier,
        client: &(dyn HttpClient + Send + Sync),
    ) -> Result<BinaryPackage, ResolverError> {
        (**self).resolve_package(pkg, client).await
    }
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct WebcIdentifier {
    /// The package's full name (i.e. `wasmer/wapm2pirita`).
    pub full_name: String,
    pub locator: Locator,
    /// A semver-compliant version constraint.
    pub version: VersionReq,
}

impl WebcIdentifier {
    pub fn parse(ident: &str) -> Result<Self, anyhow::Error> {
        ident.parse()
    }
}

impl FromStr for WebcIdentifier {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // TODO: Replace this with something more rigorous that can also handle
        // the locator field
        let (full_name, version) = match s.split_once('@') {
            Some((n, v)) => (n, v),
            None => (s, "*"),
        };

        let invalid_character = full_name
            .char_indices()
            .find(|(_, c)| !matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '.'| '-'|'_' | '/'));
        if let Some((index, c)) = invalid_character {
            anyhow::bail!("Invalid character, {c:?}, at offset {index}");
        }

        let version = version
            .parse()
            .with_context(|| format!("Invalid version number, \"{version}\""))?;

        Ok(WebcIdentifier {
            full_name: full_name.to_string(),
            locator: Locator::Registry,
            version,
        })
    }
}

impl Display for WebcIdentifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let WebcIdentifier {
            full_name,
            locator,
            version,
        } = self;

        write!(f, "{full_name}@{version}")?;

        match locator {
            Locator::Registry => {}
            Locator::Local(path) => write!(f, " ({})", path.display())?,
            Locator::Url(url) => write!(f, " ({url})")?,
        }

        Ok(())
    }
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub enum Locator {
    /// The current registry.
    Registry,
    /// A package on the current machine.
    Local(PathBuf),
    /// An exact URL.
    Url(url::Url),
}

#[derive(Debug, thiserror::Error)]
pub enum ResolverError {
    #[error("Unknown package, {_0}")]
    UnknownPackage(WebcIdentifier),
    #[error(transparent)]
    Other(Box<dyn std::error::Error + Send + Sync>),
}

#[derive(Debug, Clone)]
pub struct ResolvedPackage {
    pub commands: BTreeMap<String, ResolvedCommand>,
    pub entrypoint: Option<String>,
    /// A mapping from paths to the volumes that should be mounted there.
    pub filesystem: Vec<FileSystemMapping>,
}

impl From<ResolvedPackage> for BinaryPackage {
    fn from(_: ResolvedPackage) -> Self {
        todo!()
    }
}

impl From<BinaryPackage> for ResolvedPackage {
    fn from(_: BinaryPackage) -> Self {
        todo!()
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ResolvedCommand {
    pub metadata: webc::metadata::Command,
}

#[derive(Debug, Clone)]
pub struct FileSystemMapping {
    pub mount_path: PathBuf,
    pub volume: webc::compat::Volume,
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[test]
    fn parse_some_webc_identifiers() {
        let inputs = [
            (
                "first",
                WebcIdentifier {
                    full_name: "first".to_string(),
                    locator: Locator::Registry,
                    version: VersionReq::STAR,
                },
            ),
            (
                "namespace/package",
                WebcIdentifier {
                    full_name: "namespace/package".to_string(),
                    locator: Locator::Registry,
                    version: VersionReq::STAR,
                },
            ),
            (
                "namespace/package@1.0.0",
                WebcIdentifier {
                    full_name: "namespace/package".to_string(),
                    locator: Locator::Registry,
                    version: "1.0.0".parse().unwrap(),
                },
            ),
        ];

        for (src, expected) in inputs {
            let parsed = WebcIdentifier::from_str(src).unwrap();
            assert_eq!(parsed, expected);
        }
    }
}
