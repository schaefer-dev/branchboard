use std::fmt;

use serde::Serialize;
use ts_rs::TS;

/// A git version as reported by `git version`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, TS)]
#[ts(export)]
pub struct GitVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl GitVersion {
    /// Oldest supported version: 2.41 introduced the `%(ahead-behind:)`
    /// atom for `for-each-ref`, which computes ahead/behind counts for all
    /// branches in a single process.
    pub const MIN_SUPPORTED: Self = Self::new(2, 41, 0);

    pub const fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    /// Parses the output of `git version`, e.g. `git version 2.54.0 (Apple Git-157)`
    /// or `git version 2.47.1.windows.1`.
    pub fn parse(output: &str) -> Option<Self> {
        let token = output
            .trim()
            .strip_prefix("git version ")?
            .split_whitespace()
            .next()?;
        let mut parts = token.split('.').map(str::parse::<u32>);
        let major = parts.next()?.ok()?;
        let minor = parts.next()?.ok()?;
        let patch = parts.next().and_then(Result::ok).unwrap_or(0);
        Some(Self::new(major, minor, patch))
    }
}

impl fmt::Display for GitVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_known_formats() {
        let cases = [
            (
                "git version 2.54.0 (Apple Git-157)\n",
                GitVersion::new(2, 54, 0),
            ),
            ("git version 2.47.1.windows.1", GitVersion::new(2, 47, 1)),
            ("git version 2.41.0", GitVersion::new(2, 41, 0)),
            ("git version 2.45", GitVersion::new(2, 45, 0)),
            ("git version 2.46.0.rc1", GitVersion::new(2, 46, 0)),
        ];
        for (input, expected) in cases {
            assert_eq!(GitVersion::parse(input), Some(expected), "{input:?}");
        }
    }

    #[test]
    fn rejects_garbage() {
        for input in ["", "git", "hub version 2.1.0", "git version x.y"] {
            assert_eq!(GitVersion::parse(input), None, "{input:?}");
        }
    }

    #[test]
    fn orders_versions() {
        assert!(GitVersion::new(2, 40, 9) < GitVersion::MIN_SUPPORTED);
        assert!(GitVersion::new(2, 41, 0) >= GitVersion::MIN_SUPPORTED);
        assert!(GitVersion::new(3, 0, 0) > GitVersion::new(2, 99, 99));
    }
}
