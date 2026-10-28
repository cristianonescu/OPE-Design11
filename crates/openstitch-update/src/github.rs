//! The parts of GitHub's release JSON the updater reads.

use semver::Version;
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Asset {
    pub name: String,
    pub url: String,
    pub size: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    /// `v0.1.0-alpha.3`
    pub tag: String,
    pub version: Version,
    pub name: String,
    pub prerelease: bool,
    /// The release body: markdown (our `docs/releases/<tag>.md` plus the generated change list).
    pub body: String,
    pub html_url: String,
    /// `2026-10-27T14:54:12Z`
    pub published_at: String,
    pub assets: Vec<Asset>,
}

impl Release {
    pub fn asset(&self, name: &str) -> Option<&Asset> {
        self.assets.iter().find(|a| a.name == name)
    }
}

/// Largest release body kept, in characters; GitHub caps bodies at 125 000 anyway.
const MAX_BODY_CHARS: usize = 200_000;

/// Parses `GET /repos/{owner}/{repo}/releases`. Drafts and tags that are not release tags
/// are dropped; a malformed entry is dropped rather than failing the whole list.
pub fn parse_releases(json: &str) -> Result<Vec<Release>, String> {
    let v: Value = serde_json::from_str(json).map_err(|e| format!("release list: {e}"))?;
    let list = v
        .as_array()
        .ok_or_else(|| "release list: not a JSON array".to_string())?;
    Ok(list.iter().filter_map(parse_release).collect())
}

fn parse_release(v: &Value) -> Option<Release> {
    if v["draft"].as_bool().unwrap_or(true) {
        return None;
    }
    let tag = v["tag_name"].as_str()?.to_string();
    let version = crate::version::parse_tag(&tag)?;
    let s = |k: &str| v[k].as_str().unwrap_or("").to_string();
    let mut body = s("body");
    if body.chars().count() > MAX_BODY_CHARS {
        body = body.chars().take(MAX_BODY_CHARS).collect();
    }
    let assets = v["assets"]
        .as_array()
        .map(|list| {
            list.iter()
                .filter_map(|a| {
                    Some(Asset {
                        name: a["name"].as_str()?.to_string(),
                        url: a["browser_download_url"].as_str()?.to_string(),
                        size: a["size"].as_u64()?,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    Some(Release {
        tag,
        version,
        name: s("name"),
        prerelease: v["prerelease"].as_bool().unwrap_or(false),
        body,
        html_url: s("html_url"),
        published_at: s("published_at"),
        assets,
    })
}

#[cfg(test)]
pub(crate) const SAMPLE: &str = r##"[
 {"tag_name":"v0.1.0-alpha.4","name":"OpenStitch v0.1.0-alpha.4","draft":true,"prerelease":true,
  "html_url":"https://github.com/cristianonescu/OpenStitch/releases/tag/v0.1.0-alpha.4","body":"draft","assets":[]},
 {"tag_name":"v0.1.0-alpha.3","name":"OpenStitch v0.1.0-alpha.3","draft":false,"prerelease":true,
  "published_at":"2026-10-28T10:00:00Z",
  "html_url":"https://github.com/cristianonescu/OpenStitch/releases/tag/v0.1.0-alpha.3",
  "body":"# OpenStitch 0.1.0 alpha 3\n\nNotes for alpha 3.",
  "assets":[
   {"name":"OpenStitch-v0.1.0-alpha.3-windows-x86_64.zip","size":11487704,
    "browser_download_url":"https://github.com/cristianonescu/OpenStitch/releases/download/v0.1.0-alpha.3/OpenStitch-v0.1.0-alpha.3-windows-x86_64.zip"},
   {"name":"OpenStitch-v0.1.0-alpha.3-windows-x86_64.msi","size":11153408,
    "browser_download_url":"https://github.com/cristianonescu/OpenStitch/releases/download/v0.1.0-alpha.3/OpenStitch-v0.1.0-alpha.3-windows-x86_64.msi"},
   {"name":"OpenStitch-v0.1.0-alpha.3-macos-universal.zip","size":22181157,
    "browser_download_url":"https://github.com/cristianonescu/OpenStitch/releases/download/v0.1.0-alpha.3/OpenStitch-v0.1.0-alpha.3-macos-universal.zip"},
   {"name":"OpenStitch-v0.1.0-alpha.3-linux-x86_64.AppImage","size":16153080,
    "browser_download_url":"https://github.com/cristianonescu/OpenStitch/releases/download/v0.1.0-alpha.3/OpenStitch-v0.1.0-alpha.3-linux-x86_64.AppImage"},
   {"name":"OpenStitch-v0.1.0-alpha.3-linux-x86_64.tar.gz","size":16761983,
    "browser_download_url":"https://github.com/cristianonescu/OpenStitch/releases/download/v0.1.0-alpha.3/OpenStitch-v0.1.0-alpha.3-linux-x86_64.tar.gz"},
   {"name":"SHA256SUMS","size":700,
    "browser_download_url":"https://github.com/cristianonescu/OpenStitch/releases/download/v0.1.0-alpha.3/SHA256SUMS"}
  ]},
 {"tag_name":"v0.1.0-alpha.2","name":"OpenStitch v0.1.0-alpha.2","draft":false,"prerelease":true,
  "published_at":"2026-10-27T14:54:12Z",
  "html_url":"https://github.com/cristianonescu/OpenStitch/releases/tag/v0.1.0-alpha.2",
  "body":"# OpenStitch 0.1.0 alpha 2\n\nNotes for alpha 2.",
  "assets":[{"name":"OpenStitch-v0.1.0-alpha.2-windows-x86_64.zip","size":1,
    "browser_download_url":"https://github.com/cristianonescu/OpenStitch/releases/download/v0.1.0-alpha.2/OpenStitch-v0.1.0-alpha.2-windows-x86_64.zip"}]},
 {"tag_name":"nightly","name":"Nightly","draft":false,"prerelease":true,"body":"","assets":[]},
 {"tag_name":"v0.1.0-alpha.1","name":"OpenStitch v0.1.0-alpha.1","draft":false,"prerelease":true,
  "published_at":"2026-10-19T20:58:46Z","html_url":"","body":"Notes for alpha 1.","assets":[]}
]"##;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_published_releases_and_skips_drafts_and_odd_tags() {
        let list = parse_releases(SAMPLE).expect("parse");
        let tags: Vec<&str> = list.iter().map(|r| r.tag.as_str()).collect();
        assert_eq!(tags, ["v0.1.0-alpha.3", "v0.1.0-alpha.2", "v0.1.0-alpha.1"]);
        let a3 = &list[0];
        assert!(a3.prerelease);
        assert_eq!(a3.assets.len(), 6);
        assert_eq!(
            a3.asset("SHA256SUMS").map(|a| a.size),
            Some(700),
            "checksum asset present"
        );
        assert!(a3.body.starts_with("# OpenStitch 0.1.0 alpha 3"));
        assert_eq!(a3.published_at, "2026-10-28T10:00:00Z");
        assert_eq!(list[2].html_url, "");
    }

    #[test]
    fn rejects_non_lists_and_tolerates_junk_entries() {
        assert!(parse_releases("{}").is_err());
        assert!(parse_releases("not json").is_err());
        let junk =
            r#"[1, null, {"tag_name": 5}, {"tag_name":"v1.0.0","draft":false,"assets":"no"}]"#;
        let list = parse_releases(junk).expect("parse");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].tag, "v1.0.0");
        assert!(list[0].assets.is_empty());
    }
}
