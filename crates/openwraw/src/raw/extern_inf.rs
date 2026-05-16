// Parser for _extern.inf - the ASCII instrument parameter file present in
// every Waters .raw directory.  Provides the geometry constants (Lteff, Veff)
// and pusher timing (PusherInterval / Pusher Cycle Time) required to convert
// stored TOF bin indices into calibrated flight times and then into m/z values.

use std::collections::BTreeMap;
use std::path::Path;

const M_PROTON_KG: f64 = 1.672_621_9e-27;
const E_COULOMBS: f64 = 1.602_176_6e-19;

/// Per-function acquisition parameters extracted from a
/// `Function Parameters - Function N - TYPE` section.
#[derive(Debug, Clone)]
pub struct ExternFunction {
    /// 1-based function index.
    pub index: u32,
    /// Acquisition m/z lower limit (Da).
    pub start_mass_da: f64,
    /// Acquisition m/z upper limit (Da).
    pub end_mass_da: f64,
    /// Per-function pusher interval override (µs) from `ADC Pusher Frequency`.
    /// `None` when not present; fall back to `ExternInf::pusher_interval_us`.
    pub pusher_interval_us: Option<f64>,
}

/// Parsed contents of a Waters `_extern.inf` file.
#[derive(Debug, Clone)]
pub struct ExternInf {
    /// Effective TOF flight path length (mm).
    pub lteff_mm: f64,
    /// Effective accelerating voltage (V).
    pub veff_v: f64,
    /// Global pusher interval (µs).
    ///
    /// Sourced from `PusherInterval` (newer instruments) or
    /// `Pusher Cycle Time` (older instruments; "Automatic" is ignored).
    pub pusher_interval_us: f64,
    /// Per-function parameters keyed by 1-based function index.
    pub functions: BTreeMap<u32, ExternFunction>,
}

impl ExternInf {
    /// Read and parse an `_extern.inf` file.
    pub fn from_path(path: &Path) -> crate::Result<Self> {
        let bytes = std::fs::read(path)?;
        // Windows-1252 bytes (µ=0xB5, °=0xB0) decoded lossy; they only appear
        // in unit suffixes such as "(µs)" which are stripped before field matching.
        let text = String::from_utf8_lossy(&bytes);
        text.parse()
    }

    /// Compute the TOF constant A (µs / sqrt(Da)).
    ///
    /// Relates calibrated flight time to m/z:
    /// `t_cal_us = A_us * sqrt(mz)` therefore `mz = (t_cal_us / A_us)^2`.
    pub fn a_us(&self) -> f64 {
        let lteff_m = self.lteff_mm * 1e-3;
        lteff_m * (M_PROTON_KG / (2.0 * E_COULOMBS * self.veff_v)).sqrt() * 1e6
    }

    /// Return the effective pusher interval (µs) for a 1-based function index.
    ///
    /// Uses the per-function `ADC Pusher Frequency` override when present,
    /// otherwise returns the global `pusher_interval_us`.
    pub fn pusher_interval_for(&self, func: u32) -> f64 {
        self.functions
            .get(&func)
            .and_then(|f| f.pusher_interval_us)
            .unwrap_or(self.pusher_interval_us)
    }
}

impl std::str::FromStr for ExternInf {
    type Err = crate::Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        let mut lteff_mm: Option<f64> = None;
        let mut veff_v: Option<f64> = None;
        // Newer instruments: PusherInterval in Instrument Configuration section.
        let mut pusher_from_interval: Option<f64> = None;
        // Older instruments: Pusher Cycle Time in Instrument Parameters section.
        let mut pusher_from_cycle: Option<f64> = None;

        let mut current_func: Option<u32> = None;
        let mut functions: BTreeMap<u32, ExternFunction> = BTreeMap::new();

        for line in s.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            // Detect "Function Parameters - Function N - TYPE" section headers.
            // These do NOT end with ':'.
            if let Some(rest) = trimmed.strip_prefix("Function Parameters - Function ") {
                // rest = "1 - TOF MS FUNCTION" or similar
                let n_str = rest.split_whitespace().next().unwrap_or("");
                if let Ok(n) = n_str.trim_end_matches('-').trim().parse::<u32>() {
                    current_func = Some(n);
                    functions.entry(n).or_insert(ExternFunction {
                        index: n,
                        start_mass_da: 0.0,
                        end_mass_da: 0.0,
                        pusher_interval_us: None,
                    });
                }
                continue;
            }

            // Any other section header ends with ':'.
            // "Instrument Parameters - Function N:" (older format) contains Lteff/Veff
            // as global constants, so we reset current_func to parse them as global.
            if trimmed.ends_with(':') {
                if trimmed.starts_with("Instrument") || !trimmed.contains("Function") {
                    current_func = None;
                }
                continue;
            }

            // Field line: split by whitespace; last token = value, rest = key.
            let tokens: Vec<&str> = trimmed.split_whitespace().collect();
            if tokens.len() < 2 {
                continue;
            }
            let value_str = tokens[tokens.len() - 1];
            let key: String = tokens[..tokens.len() - 1].join(" ");

            // Strip unit suffixes in parentheses (e.g. "(µs)", "(°C)") before matching.
            // split('(') on "Pusher Cycle Time (µs)" yields "Pusher Cycle Time ".
            let key_base = key.split('(').next().unwrap_or(key.as_str()).trim_end();

            match key_base {
                "Lteff" => {
                    if let Ok(v) = value_str.parse::<f64>() {
                        lteff_mm.get_or_insert(v);
                    }
                }
                "Veff" => {
                    if let Ok(v) = value_str.parse::<f64>() {
                        veff_v.get_or_insert(v);
                    }
                }
                "PusherInterval" => {
                    if let Ok(v) = value_str.parse::<f64>() {
                        pusher_from_interval.get_or_insert(v);
                    }
                }
                "Pusher Cycle Time" => {
                    // Value may be "Automatic"; ignore that.
                    if let Ok(v) = value_str.parse::<f64>() {
                        if value_str != "Automatic" {
                            pusher_from_cycle.get_or_insert(v);
                        }
                    }
                }
                "Start Mass" => {
                    if let (Some(n), Ok(v)) = (current_func, value_str.parse::<f64>()) {
                        if let Some(f) = functions.get_mut(&n) {
                            f.start_mass_da = v;
                        }
                    }
                }
                "End Mass" => {
                    if let (Some(n), Ok(v)) = (current_func, value_str.parse::<f64>()) {
                        if let Some(f) = functions.get_mut(&n) {
                            f.end_mass_da = v;
                        }
                    }
                }
                "ADC Pusher Frequency" => {
                    if let (Some(n), Ok(v)) = (current_func, value_str.parse::<f64>()) {
                        if let Some(f) = functions.get_mut(&n) {
                            f.pusher_interval_us = Some(v);
                        }
                    }
                }
                _ => {}
            }
        }

        let lteff_mm = lteff_mm
            .ok_or_else(|| crate::Error::Parse("_extern.inf: Lteff field not found".to_owned()))?;
        let veff_v = veff_v
            .ok_or_else(|| crate::Error::Parse("_extern.inf: Veff field not found".to_owned()))?;
        // Prefer the dedicated PusherInterval field; fall back to Pusher Cycle Time.
        let pusher_interval_us = pusher_from_interval.or(pusher_from_cycle).ok_or_else(|| {
            crate::Error::Parse(
                "_extern.inf: neither PusherInterval nor Pusher Cycle Time found".to_owned(),
            )
        })?;

        Ok(ExternInf {
            lteff_mm,
            veff_v,
            pusher_interval_us,
            functions,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // PXD058812 (Q-TOF Ultima, older format): Lteff/Veff inside
    // "Instrument Parameters - Function 1:" section; pusher from "Pusher Cycle Time".
    const EXTERN_PXD058812: &str = "\
Parameters for C:\\MassLynx\\Qtof\\tuneexp.exp\r\n\
 \r\n\
Instrument Parameters - Function 1:\r\n\
Lteff\t1997.9400\r\n\
Veff\t9100.0000\r\n\
Pusher Cycle Time (µs)	62
\
 \r\n\
Function Parameters - Function 1 - TOF MS FUNCTION\r\n\
Start Mass                              100.0\r\n\
End Mass                                2000.0\r\n\
Scan Time (sec)                         1.0\r\n\
";

    // PXD075602 (Xevo G2-XS QTof, newer format): Lteff/Veff and PusherInterval
    // in "Instrument Configuration:" section; per-function ADC Pusher Frequency override.
    const EXTERN_PXD075602: &str = "\
Parameters for D:\\Projects\\method.EXP\r\n\
Created by MassLynx v4.2 SCN966\r\n\
 \r\n\
Instrument Configuration:\r\n\
Lteff                                           1800.0\r\n\
Veff                                            6328.24\r\n\
PusherInterval                                  60.250000\r\n\
 \r\n\
Function Parameters - Function 1 - TOF PARENT FUNCTION\r\n\
Start Mass                                      50.0\r\n\
End Mass                                        1200.0\r\n\
Survey Scan Time                                0.5\r\n\
ADC Pusher Frequency (µs)                    60.3\r\n\
 \r\n\
Function Parameters - Function 2 - TOF PARENT FUNCTION\r\n\
Start Mass                                      50.0\r\n\
End Mass                                        1200.0\r\n\
Survey Scan Time                                0.5\r\n\
ADC Pusher Frequency (µs)                    60.3\r\n\
 \r\n\
Function Parameters - Function 3 - TOF PRODUCT FUNCTION\r\n\
Start Mass                                      50.0\r\n\
End Mass                                        1200.0\r\n\
Survey Scan Time                                0.5\r\n\
ADC Pusher Frequency (µs)                    60.3\r\n\
";

    // PXD068881 (Synapt G2-Si IMS): PusherInterval = 69.0, no per-function override.
    const EXTERN_PXD068881: &str = "\
Parameters for method.EXP\r\n\
Created by 4.1 SCN 965\r\n\
 \r\n\
Instrument Configuration:\r\n\
Lteff                                          1800.0\r\n\
Veff                                           7198.65\r\n\
PusherInterval                                 69.000000\r\n\
 \r\n\
Function Parameters - Function 1 - TOF PARENT FUNCTION\r\n\
Start Mass                                     50.0\r\n\
End Mass                                       2000.0\r\n\
";

    #[test]
    fn parse_older_format_pusher_cycle_time() {
        let ext: ExternInf = EXTERN_PXD058812.parse().unwrap();
        assert!((ext.lteff_mm - 1997.94).abs() < 1e-3);
        assert!((ext.veff_v - 9100.0).abs() < 1e-3);
        assert!((ext.pusher_interval_us - 62.0).abs() < 1e-6);
    }

    #[test]
    fn parse_newer_format_pusher_interval() {
        let ext: ExternInf = EXTERN_PXD075602.parse().unwrap();
        assert!((ext.lteff_mm - 1800.0).abs() < 1e-3);
        assert!((ext.veff_v - 6328.24).abs() < 1e-3);
        assert!((ext.pusher_interval_us - 60.25).abs() < 1e-6);
    }

    #[test]
    fn parse_ims_instrument() {
        let ext: ExternInf = EXTERN_PXD068881.parse().unwrap();
        assert!((ext.lteff_mm - 1800.0).abs() < 1e-3);
        assert!((ext.veff_v - 7198.65).abs() < 1e-3);
        assert!((ext.pusher_interval_us - 69.0).abs() < 1e-6);
    }

    #[test]
    fn parse_function_mass_range() {
        let ext: ExternInf = EXTERN_PXD058812.parse().unwrap();
        let f1 = ext.functions.get(&1).expect("Function 1 missing");
        assert!((f1.start_mass_da - 100.0).abs() < 1e-6);
        assert!((f1.end_mass_da - 2000.0).abs() < 1e-6);
        assert!(f1.pusher_interval_us.is_none());
    }

    #[test]
    fn parse_per_function_pusher_override() {
        let ext: ExternInf = EXTERN_PXD075602.parse().unwrap();
        assert_eq!(ext.functions.len(), 3);
        for n in 1..=3u32 {
            let f = ext
                .functions
                .get(&n)
                .unwrap_or_else(|| panic!("Function {n} missing"));
            let ov = f.pusher_interval_us.expect("ADC Pusher Frequency missing");
            assert!((ov - 60.3).abs() < 1e-6);
        }
    }

    #[test]
    fn pusher_interval_for_falls_back_to_global() {
        let ext: ExternInf = EXTERN_PXD068881.parse().unwrap();
        // No per-function override -> global value returned.
        assert!((ext.pusher_interval_for(1) - 69.0).abs() < 1e-6);
    }

    #[test]
    fn pusher_interval_for_uses_per_function_override() {
        let ext: ExternInf = EXTERN_PXD075602.parse().unwrap();
        // Per-function ADC Pusher Frequency = 60.3, global = 60.25.
        assert!((ext.pusher_interval_for(1) - 60.3).abs() < 1e-6);
    }

    #[test]
    fn a_us_plausible_range() {
        // A_us for all known corpus datasets should fall in [1.0, 3.0] µs/sqrt(Da).
        // Verified formula: A_us = Lteff_m * sqrt(m_proton / (2 * e * Veff)) * 1e6
        let ext_58812: ExternInf = EXTERN_PXD058812.parse().unwrap();
        let ext_75602: ExternInf = EXTERN_PXD075602.parse().unwrap();
        let ext_68881: ExternInf = EXTERN_PXD068881.parse().unwrap();

        for (name, ext) in [
            ("PXD058812", &ext_58812),
            ("PXD075602", &ext_75602),
            ("PXD068881", &ext_68881),
        ] {
            let a = ext.a_us();
            assert!(
                (1.0..3.0).contains(&a),
                "{name}: A_us={a} outside expected range [1.0, 3.0]"
            );
        }
    }

    #[test]
    fn a_us_formula_pxd058812() {
        // Manually computed:
        // A_us = 1.99794 * sqrt(1.6726219e-27 / (2 * 1.6021766e-19 * 9100)) * 1e6
        //      = 1.99794 * 7.5731e-7 * 1e6 ≈ 1.5129 µs/sqrt(Da)
        let ext: ExternInf = EXTERN_PXD058812.parse().unwrap();
        let a = ext.a_us();
        assert!(
            (a - 1.5129).abs() < 1e-3,
            "A_us={a}, expected ≈1.5129 µs/sqrt(Da)"
        );
    }

    #[test]
    fn missing_lteff_is_error() {
        let src = "Veff  9100.0\r\nPusherInterval  88.0\r\n";
        let err = src.parse::<ExternInf>().unwrap_err();
        assert!(err.to_string().contains("Lteff"));
    }

    #[test]
    fn missing_pusher_is_error() {
        let src = "Lteff  1997.94\r\nVeff  9100.0\r\n";
        let err = src.parse::<ExternInf>().unwrap_err();
        assert!(err.to_string().contains("PusherInterval"));
    }
}
