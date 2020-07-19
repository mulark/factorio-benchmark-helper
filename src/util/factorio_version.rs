//! Structure for easily comparing different Factorio versions
use serde::Serialize;
use serde::Deserialize;
use std::cmp::Ordering;
use std::convert::TryFrom;
use std::convert::TryInto;

/// A Factorio version consisting of a major, minor and patch version
#[serde(into = "String")]
#[serde(try_from = "&str")]
#[derive(Debug, Clone, PartialEq, Eq, Ord, Default, Serialize, Deserialize)]
pub struct FactorioVersion {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

impl FactorioVersion {
    fn is_known(&self) -> bool {
        self == &("0.0.0".try_into().unwrap())
    }
}

impl Into<String> for FactorioVersion {
    fn into(self) -> String {
        self.to_string()
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

impl core::fmt::Display for FactorioVersion {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl TryFrom<&str> for FactorioVersion {
    type Error = String;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        let splits = s.split('.').collect::<Vec<_>>();
        if splits.len() != 3 {
            return Err("Incorrect number of periods present in version string".to_owned());
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
            Err("Unparseable/non-numeric data found within version subsection!".to_owned())
        }
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
