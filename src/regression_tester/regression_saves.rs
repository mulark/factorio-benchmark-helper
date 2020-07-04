//! Module for defining saves to be regression tested.
use crate::util::FactorioVersion;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct FactorioSave {
    pub name: String,
    pub download_url: String,
    pub sha256: String,
    /// The version at which this map was saved. Prevents loading in earlier
    /// versions.
    pub compatible_version: FactorioVersion,
}

/// A range for which recipes did not change
pub struct RecipeRange {
    first_version: FactorioVersion,
    final_version: FactorioVersion,
}

#[cfg(test)]
mod tests {
    use super::FactorioSave;
    use std::convert::TryInto;

    #[test]
    fn test_serialize_saves() {
        let saves = vec![
            FactorioSave {
                name: String::from("test-000050.empty_filter.zip"),
                download_url: String::from("https://f000.backblazeb2.com/file/mulark-maps/test-000050/test-000050.empty_filter.zip"),
                sha256: String::from("94498095e5696c3279f5d5a7ad18f2a44401a94ac9c4d52191a71ef74a08adc8"),
                compatible_version: "0.18.17".try_into().unwrap(),
            },
            FactorioSave {
                name: String::from("test-000050.empty_nofilter.zip"),
                download_url: String::from("https://f000.backblazeb2.com/file/mulark-maps/test-000050/test-000050.empty_nofilter.zip"),
                sha256: String::from("73083a980654734dc96e6a12fbb297fb35ca91467cd36c08f299a30e0a1a43d0"),
                compatible_version: "0.18.17".try_into().unwrap(),
            },
        ];
        let s = serde_json::to_string_pretty(&saves).unwrap();
        eprintln!("{:?}", s);
        assert_eq!(
            "[\n  {\n    \"name\": \"test-000050.empty_filter.zip\",\n    \"download_url\": \"https://f000.backblazeb2.com/file/mulark-maps/test-000050/test-000050.empty_filter.zip\",\n    \"sha256\": \"94498095e5696c3279f5d5a7ad18f2a44401a94ac9c4d52191a71ef74a08adc8\",\n    \"compatible_version\": \"0.18.17\"\n  },\n  {\n    \"name\": \"test-000050.empty_nofilter.zip\",\n    \"download_url\": \"https://f000.backblazeb2.com/file/mulark-maps/test-000050/test-000050.empty_nofilter.zip\",\n    \"sha256\": \"73083a980654734dc96e6a12fbb297fb35ca91467cd36c08f299a30e0a1a43d0\",\n    \"compatible_version\": \"0.18.17\"\n  }\n]",
            s
        );
    }
}
