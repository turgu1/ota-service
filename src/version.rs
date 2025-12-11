use std::cmp::Ordering;

/// Version number represented as three integers (major.minor.patch)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl Version {
    /// Parse a version string in the format "major.minor.patch"
    ///
    /// # Arguments
    /// * `version_str` - Version string (e.g., "1.2.3")
    ///
    /// # Returns
    /// Result containing Version or error message
    pub fn parse(version_str: &str) -> Result<Self, String> {
        let parts: Vec<&str> = version_str.split('.').collect();

        if parts.len() != 3 {
            return Err(format!(
                "Invalid version format '{}': expected 'major.minor.patch'",
                version_str
            ));
        }

        let major = parts[0]
            .parse::<u32>()
            .map_err(|_| format!("Invalid major version: '{}'", parts[0]))?;

        let minor = parts[1]
            .parse::<u32>()
            .map_err(|_| format!("Invalid minor version: '{}'", parts[1]))?;

        let patch = parts[2]
            .parse::<u32>()
            .map_err(|_| format!("Invalid patch version: '{}'", parts[2]))?;

        Ok(Version {
            major,
            minor,
            patch,
        })
    }

    /// Compare this version with another version
    ///
    /// # Arguments
    /// * `other` - Other version to compare with
    ///
    /// # Returns
    /// Ordering (Less, Equal, or Greater)
    pub fn compare(&self, other: &Version) -> Ordering {
        match self.major.cmp(&other.major) {
            Ordering::Equal => match self.minor.cmp(&other.minor) {
                Ordering::Equal => self.patch.cmp(&other.patch),
                other => other,
            },
            other => other,
        }
    }

    /// Check if this version is greater than another version
    pub fn is_greater_than(&self, other: &Version) -> bool {
        self.compare(other) == Ordering::Greater
    }

    /// Check if this version is less than another version
    pub fn is_less_than(&self, other: &Version) -> bool {
        self.compare(other) == Ordering::Less
    }

    /// Check if this version is less than or equal to another version
    pub fn is_less_or_equal(&self, other: &Version) -> bool {
        matches!(self.compare(other), Ordering::Less | Ordering::Equal)
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.compare(other))
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        self.compare(other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_parse() {
        let v = Version::parse("1.2.3").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
    }

    #[test]
    fn test_version_parse_invalid() {
        assert!(Version::parse("1.2").is_err());
        assert!(Version::parse("1.2.3.4").is_err());
        assert!(Version::parse("a.b.c").is_err());
        assert!(Version::parse("1.2.x").is_err());
    }

    #[test]
    fn test_version_compare() {
        let v1 = Version::parse("1.2.3").unwrap();
        let v2 = Version::parse("1.2.4").unwrap();
        let v3 = Version::parse("1.3.0").unwrap();
        let v4 = Version::parse("2.0.0").unwrap();
        let v5 = Version::parse("1.2.3").unwrap();

        assert_eq!(v1.compare(&v2), Ordering::Less);
        assert_eq!(v2.compare(&v1), Ordering::Greater);
        assert_eq!(v1.compare(&v5), Ordering::Equal);
        assert!(v1.is_less_than(&v2));
        assert!(v1.is_less_than(&v3));
        assert!(v1.is_less_than(&v4));
        assert!(v2.is_greater_than(&v1));
        assert!(v4.is_greater_than(&v3));
    }

    #[test]
    fn test_version_numeric_comparison() {
        // Test that "1.10.0" > "1.9.0" (not string comparison where "1.9" > "1.10")
        let v1 = Version::parse("1.9.0").unwrap();
        let v2 = Version::parse("1.10.0").unwrap();
        assert!(v2.is_greater_than(&v1));
        assert_eq!(v1.compare(&v2), Ordering::Less);

        // Test "2.0.0" > "1.99.99"
        let v3 = Version::parse("1.99.99").unwrap();
        let v4 = Version::parse("2.0.0").unwrap();
        assert!(v4.is_greater_than(&v3));
    }
}
