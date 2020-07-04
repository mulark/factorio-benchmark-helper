//! Structure for easily comparing different Factorio versions

use serde::de::Visitor;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use serde::Serializer;
use std::cmp::Ordering;
use std::convert::TryFrom;

/// A Factorio version consisting of a major, minor and patch version
#[derive(Debug, Clone)]
pub struct FactorioVersion {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

struct FactorioVersionVisitor;

impl<'de> Visitor<'de> for FactorioVersionVisitor {
    type Value = FactorioVersion;
    fn expecting(
        &self,
        formatter: &mut std::fmt::Formatter<'_>,
    ) -> Result<(), std::fmt::Error> {
        formatter.write_str("")
    }

    fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        if let Ok(ret) = FactorioVersion::try_from(s) {
            Ok(ret)
        } else {
            Err(E::custom("Could not deserialize FactorioVersion"))
        }
    }
}

impl FactorioVersion {
    pub fn new(major: u16, minor: u16, patch: u16) -> FactorioVersion {
        FactorioVersion {
            major,
            minor,
            patch,
        }
    }
}

impl Serialize for FactorioVersion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for FactorioVersion {
    fn deserialize<D>(deserializer: D) -> Result<FactorioVersion, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(FactorioVersionVisitor)
    }
}

impl core::fmt::Display for FactorioVersion {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl TryFrom<&str> for FactorioVersion {
    type Error = ();
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        let splits = s.split('.').collect::<Vec<_>>();
        if splits.len() != 3 {
            return Err(());
        }
        let splits = splits.iter().map(|x| x.parse()).collect::<Vec<_>>();
        if splits.iter().all(|x| x.is_ok()) {
            let splits = splits
                .iter()
                .map(|x| *(x.as_ref().unwrap()))
                .collect::<Vec<_>>();
            Ok(FactorioVersion {
                major: splits[0],
                minor: splits[1],
                patch: splits[2],
            })
        } else {
            Err(())
        }
    }
}

impl PartialEq for FactorioVersion {
    fn eq(&self, other: &FactorioVersion) -> bool {
        self.major == other.major
            && self.minor == other.minor
            && self.patch == other.patch
    }
}

impl PartialOrd for FactorioVersion {
    fn partial_cmp(&self, other: &FactorioVersion) -> Option<Ordering> {
        if self.major > other.major {
            Some(Ordering::Greater)
        } else if self.major < other.major {
            Some(Ordering::Less)
        } else if self.minor > other.minor {
            Some(Ordering::Greater)
        } else if self.minor < other.minor {
            Some(Ordering::Less)
        } else if self.patch > other.patch {
            Some(Ordering::Greater)
        } else if self.patch < other.patch {
            Some(Ordering::Less)
        } else {
            Some(Ordering::Equal)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_ser_fv() {
        let fv = FactorioVersion::new(0,17,79);
        let serialized = serde_json::to_string(&fv).unwrap();
        assert_eq!(serialized, "\"0.17.79\"");
    }

    #[test]
    fn test_deser_fv() {
        let fv = serde_json::from_str::<FactorioVersion>("\"0.17.79\"").unwrap();
        let fv_reference = FactorioVersion::new(0,17,79);
        assert_eq!(fv, fv_reference);
    }

    #[test]
    fn test_ser_fv_deser_fv_roundtrip() {
        let fv = FactorioVersion::new(0,17,79);
        let serialized = serde_json::to_string(&fv).unwrap();
        let deserialized = serde_json::from_str::<FactorioVersion>(&serialized).unwrap();
        assert_eq!(deserialized, fv);
    }
}
