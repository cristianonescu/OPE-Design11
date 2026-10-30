//! Transcendental functions that give the same bits on every platform.
//!
//! Rust's `f64::sin`, `cos`, `atan2`, `hypot` and the like call the platform's C math library
//! (glibc on Linux, the MSVC runtime on Windows, Apple's libm on macOS), and those differ in
//! the last bit for some arguments. Stitch generation turns such differences into different
//! output where a value sits on a threshold (a row count from `floor`, a stitch split from
//! `ceil`): an arched text measured 25.8 mm high on Linux and 26.0 mm on Windows and macOS
//! (compat spec pes-0110-editable-text-transform, 2026-10-29). These methods use the `libm`
//! crate, a pure-Rust port of musl's libm, so a design sews the same stitches everywhere.
//!
//! `sqrt`, `abs`, `floor`, `ceil`, `round`, `mul_add` and the arithmetic operators are exact
//! by IEEE 754 and need no replacement. A test in this module keeps the crate's code off the
//! platform functions.

/// Platform-independent transcendental functions on `f64` (see the module documentation).
pub trait Det {
    fn dsin(self) -> f64;
    fn dcos(self) -> f64;
    fn dsin_cos(self) -> (f64, f64);
    fn dtan(self) -> f64;
    fn dasin(self) -> f64;
    fn dacos(self) -> f64;
    fn datan(self) -> f64;
    fn datan2(self, x: f64) -> f64;
    fn dhypot(self, other: f64) -> f64;
    fn dexp(self) -> f64;
    fn dln(self) -> f64;
    fn dlog10(self) -> f64;
    fn dpowf(self, n: f64) -> f64;
    fn dcbrt(self) -> f64;
}

impl Det for f64 {
    fn dsin(self) -> f64 {
        libm::sin(self)
    }
    fn dcos(self) -> f64 {
        libm::cos(self)
    }
    fn dsin_cos(self) -> (f64, f64) {
        libm::sincos(self)
    }
    fn dtan(self) -> f64 {
        libm::tan(self)
    }
    fn dasin(self) -> f64 {
        libm::asin(self)
    }
    fn dacos(self) -> f64 {
        libm::acos(self)
    }
    fn datan(self) -> f64 {
        libm::atan(self)
    }
    /// `self.datan2(x)` is the angle of (x, self), as `f64::atan2`.
    fn datan2(self, x: f64) -> f64 {
        libm::atan2(self, x)
    }
    fn dhypot(self, other: f64) -> f64 {
        libm::hypot(self, other)
    }
    fn dexp(self) -> f64 {
        libm::exp(self)
    }
    fn dln(self) -> f64 {
        libm::log(self)
    }
    fn dlog10(self) -> f64 {
        libm::log10(self)
    }
    fn dpowf(self, n: f64) -> f64 {
        libm::pow(self, n)
    }
    fn dcbrt(self) -> f64 {
        libm::cbrt(self)
    }
}

#[cfg(test)]
mod tests {
    use super::Det;

    #[test]
    fn deterministic_functions_agree_with_the_platform_within_an_ulp_or_two() {
        let close = |a: f64, b: f64| (a - b).abs() <= 4.0 * f64::EPSILON * a.abs().max(1.0);
        for k in -40..=40 {
            let x = k as f64 * 0.173;
            assert!(close(x.dsin(), x.sin()), "{x}");
            assert!(close(x.dcos(), x.cos()), "{x}");
            assert_eq!(x.dsin_cos(), (x.dsin(), x.dcos()));
            assert!(close(x.datan2(1.3), x.atan2(1.3)), "{x}");
            assert!(close(x.dhypot(2.1), x.hypot(2.1)), "{x}");
        }
        assert_eq!(0.0f64.dsin(), 0.0);
        assert_eq!(0.0f64.dcos(), 1.0);
        assert_eq!(3.0f64.dhypot(4.0), 5.0);
    }

    /// The crate's own code calls no platform transcendental function: every such call goes
    /// through [`Det`] (tests may compare against the platform's).
    #[test]
    fn the_crate_uses_no_platform_transcendental_functions() {
        let names = [
            "sin", "cos", "sin_cos", "tan", "asin", "acos", "atan", "atan2", "sinh", "cosh",
            "tanh", "exp", "exp2", "exp_m1", "ln", "log", "log2", "log10", "ln_1p", "powf",
            "hypot", "cbrt",
        ];
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut found = Vec::new();
        for entry in std::fs::read_dir(&dir).expect("src") {
            let path = entry.expect("entry").path();
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if !name.ends_with(".rs") || name.ends_with("tests.rs") || name == "fmath.rs" {
                continue;
            }
            let text = std::fs::read_to_string(&path).expect("read");
            // Code before the file's own test module only.
            let code = text.split("#[cfg(test)]").next().unwrap_or("");
            for (n, line) in code.lines().enumerate() {
                let line = line.split("//").next().unwrap_or("");
                for f in names {
                    if line.contains(&format!(".{f}(")) {
                        found.push(format!("{name}:{}: .{f}(", n + 1));
                    }
                }
            }
        }
        assert!(found.is_empty(), "platform math calls: {found:?}");
    }
}
