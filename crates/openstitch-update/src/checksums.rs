//! The `SHA256SUMS` file `release.yml` publishes (`sha256sum` format).

use std::collections::BTreeMap;
use std::fmt;

/// A SHA-256 digest.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Sha256(pub [u8; 32]);

impl Sha256 {
    pub fn parse(hex: &str) -> Option<Sha256> {
        if hex.len() != 64 {
            return None;
        }
        let mut out = [0u8; 32];
        for (i, b) in out.iter_mut().enumerate() {
            *b = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).ok()?;
        }
        Some(Sha256(out))
    }

    pub fn of(bytes: &[u8]) -> Sha256 {
        use sha2::Digest;
        Sha256(sha2::Sha256::digest(bytes).into())
    }
}

impl fmt::Display for Sha256 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for b in self.0 {
            write!(f, "{b:02x}")?;
        }
        Ok(())
    }
}

impl fmt::Debug for Sha256 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Sha256({self})")
    }
}

/// Asset name → digest.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Checksums(pub BTreeMap<String, Sha256>);

impl Checksums {
    /// Parses `<hex>  <name>` or `<hex> *<name>` lines; blank lines are skipped, a malformed
    /// line is an error (a damaged file must not silently drop entries).
    pub fn parse(text: &str) -> Result<Checksums, String> {
        let mut map = BTreeMap::new();
        for (n, line) in text.lines().enumerate() {
            let line = line.trim_end_matches('\r');
            if line.trim().is_empty() {
                continue;
            }
            let (hex, name) = line
                .split_once(' ')
                .ok_or_else(|| format!("SHA256SUMS line {}: no separator", n + 1))?;
            let name = name.trim_start_matches([' ', '*']);
            let sum = Sha256::parse(hex)
                .ok_or_else(|| format!("SHA256SUMS line {}: bad digest", n + 1))?;
            if name.is_empty() || name.contains('/') || name.contains('\\') {
                return Err(format!("SHA256SUMS line {}: bad name", n + 1));
            }
            map.insert(name.to_string(), sum);
        }
        Ok(Checksums(map))
    }

    pub fn get(&self, name: &str) -> Option<Sha256> {
        self.0.get(name).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sha256sum_output() {
        let text = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855  OpenStitch-v0.1.0-alpha.3-windows-x86_64.zip\r\n\
                    E3B0C44298FC1C149AFBF4C8996FB92427AE41E4649B934CA495991B7852B855 *OpenStitch-v0.1.0-alpha.3-windows-x86_64.msi\n\n";
        let c = Checksums::parse(text).expect("parse");
        assert_eq!(c.0.len(), 2);
        let empty = Sha256::of(b"");
        assert_eq!(
            c.get("OpenStitch-v0.1.0-alpha.3-windows-x86_64.zip"),
            Some(empty)
        );
        assert_eq!(
            c.get("OpenStitch-v0.1.0-alpha.3-windows-x86_64.msi"),
            Some(empty)
        );
        assert_eq!(
            empty.to_string(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(c.get("other"), None);
        assert_eq!(Checksums::parse("").expect("empty"), Checksums::default());
    }

    #[test]
    fn rejects_damaged_lines() {
        for bad in [
            "abc  file",
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b85g  file",
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855  ",
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855  ../x",
        ] {
            assert!(Checksums::parse(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn digest_of_known_input() {
        assert_eq!(
            Sha256::of(b"abc").to_string(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
