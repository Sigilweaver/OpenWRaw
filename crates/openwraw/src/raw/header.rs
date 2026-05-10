// Parser for HEADER.TXT - the plaintext ASCII metadata file present in
// every Waters .raw directory. Contains instrument name, operator,
// acquisition date, sample description, and MassLynx version.

use std::collections::BTreeMap;
use std::path::Path;

/// Calibration polynomial type recorded in `Cal Function N:` lines.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum CalType {
    /// No calibration applied; raw TOF time is used unchanged.
    #[default]
    T0,
    /// Standard Waters TOF polynomial: `t_cal = c0 + c1*t + c2*t^2 + ...`
    T1,
}

/// Per-function TOF calibration polynomial from a `Cal Function N:` line.
#[derive(Debug, Clone, Default)]
pub struct FunctionCal {
    /// Coefficients `[c0, c1, c2, ...]` in ascending power order.
    /// Coefficients [c0, c1, c2, ...] in ascending power order.
    /// Default: empty (identity for T0; treated as t_raw unchanged).
    pub coeffs: Vec<f64>,
    pub cal_type: CalType,
}

impl FunctionCal {
    /// Apply the calibration polynomial to a raw TOF time (microseconds).
    ///
    /// Returns `t_raw` unchanged for `T0`.  Evaluates the polynomial via
    /// Horner's method for `T1`.
    pub fn apply(&self, t_raw: f64) -> f64 {
        match self.cal_type {
            CalType::T0 => t_raw,
            CalType::T1 => {
                let mut result = 0.0_f64;
                for &c in self.coeffs.iter().rev() {
                    result = result * t_raw + c;
                }
                result
            }
        }
    }
}

/// Parsed contents of a Waters `_HEADER.TXT` file.
#[derive(Debug, Clone, Default)]
pub struct Header {
    pub version: Option<String>,
    pub acquired_name: Option<String>,
    pub acquired_date: Option<String>,
    pub acquired_time: Option<String>,
    pub instrument: Option<String>,
    pub operator: Option<String>,
    pub sample_description: Option<String>,
    /// Calibration polynomials keyed by 1-based function index.
    pub cal_functions: BTreeMap<u32, FunctionCal>,
}

impl Header {
    /// Read and parse a `_HEADER.TXT` file.
    pub fn from_path(path: &Path) -> crate::Result<Self> {
        let bytes = std::fs::read(path)?;
        // HEADER.TXT is nominally ASCII; lossy decode handles rare non-ASCII bytes
        // in Cal Params fields without failing.
        let text = String::from_utf8_lossy(&bytes);
        text.parse()
    }
}

impl std::str::FromStr for Header {
    type Err = crate::Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        let mut header = Header::default();

        for line in s.lines() {
            // Every metadata line starts with "$$"; skip anything else
            let rest = match line.trim().strip_prefix("$$") {
                Some(r) => r.trim_start(),
                None => continue,
            };

            // Key and value are separated by the first ": "
            let (key, value) = match rest.split_once(": ") {
                Some((k, v)) => (k.trim(), v.trim()),
                None => continue,
            };

            match key {
                "Version" => header.version = Some(value.to_owned()),
                "Acquired Name" => header.acquired_name = Some(value.to_owned()),
                "Acquired Date" => header.acquired_date = Some(value.to_owned()),
                "Acquired Time" => header.acquired_time = Some(value.to_owned()),
                "Instrument" => header.instrument = Some(value.to_owned()),
                "User Name" if !value.is_empty() => {
                    header.operator = Some(value.to_owned());
                }
                "Sample Description" if !value.is_empty() => {
                    header.sample_description = Some(value.to_owned());
                }
                k if k.starts_with("Cal Function ") => {
                    let n_str = k.trim_start_matches("Cal Function ");
                    let n: u32 = n_str.trim().parse().map_err(|_| {
                        crate::Error::Parse(format!(
                            "_HEADER.TXT: invalid function index in key {k:?}"
                        ))
                    })?;
                    header.cal_functions.insert(n, parse_cal_value(value)?);
                }
                _ => {}
            }
        }

        Ok(header)
    }
}

fn parse_cal_value(value: &str) -> crate::Result<FunctionCal> {
    // Format: "c0,c1,...,ck,TYPE"
    // The last comma-separated token is the type code; all preceding are f64 coefficients.
    let parts: Vec<&str> = value.split(',').collect();

    let type_str = parts.last().map(|s| s.trim()).unwrap_or("");
    let cal_type = match type_str {
        "T0" => CalType::T0,
        "T1" => CalType::T1,
        other => {
            return Err(crate::Error::Parse(format!(
                "_HEADER.TXT: unknown calibration type {other:?}"
            )));
        }
    };

    let coeff_count = parts.len().saturating_sub(1);
    let coeffs: crate::Result<Vec<f64>> = parts[..coeff_count]
        .iter()
        .filter(|s| !s.trim().is_empty())
        .map(|s| {
            s.trim().parse::<f64>().map_err(|_| {
                crate::Error::Parse(format!("_HEADER.TXT: invalid coefficient {s:?}"))
            })
        })
        .collect();

    Ok(FunctionCal {
        coeffs: coeffs?,
        cal_type,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Coefficients from PXD058812/molecular_mass_P15_01.raw/_HEADER.TXT
    const HEADER_PXD058812: &str = "\
$$ Version: 01.00\r\n\
$$ Acquired Name: 14012021_P15_1\r\n\
$$ Acquired Date: 14-Jan-2021\r\n\
$$ Acquired Time: 16:20:52\r\n\
$$ Instrument: QTOF\r\n\
$$ User Name: \r\n\
$$ Sample Description: 1,52 ul/ul in 200 mM AAc, Quad. 500\r\n\
$$ Cal Function 1: -3.072270525784614e-2,9.999211110337035e-1,8.393242633253761e-5,-2.359940313013620e-6,2.529329369860990e-8,T1\r\n\
$$ Cal StdDev Function 1: 0.000000000000000e0\r\n\
";

    // Coefficients from PXD075602/DHPR_11257-1.raw/_HEADER.TXT (6-coefficient T1, 3 functions)
    const HEADER_PXD075602: &str = "\
$$ Version: 01.00\r\n\
$$ Instrument: XEVO-G2XSQTOF#NotSet\r\n\
$$ Cal Function 1: -4.777591233644572e-3,1.000131905522236e0,4.430892196785128e-6,-1.461064186367955e-7,-2.190603738979474e-10,5.118758341877316e-11,T1\r\n\
$$ Cal Function 2: -4.777591233644572e-3,1.000131905522236e0,4.430892196785128e-6,-1.461064186367955e-7,-2.190603738979474e-10,5.118758341877316e-11,T1\r\n\
$$ Cal Function 3: -4.777591233644572e-3,1.000131905522236e0,4.430892196785128e-6,-1.461064186367955e-7,-2.190603738979474e-10,5.118758341877316e-11,T1\r\n\
";

    #[test]
    fn parse_metadata_fields() {
        let h: Header = HEADER_PXD058812.parse().unwrap();
        assert_eq!(h.version.as_deref(), Some("01.00"));
        assert_eq!(h.acquired_name.as_deref(), Some("14012021_P15_1"));
        assert_eq!(h.acquired_date.as_deref(), Some("14-Jan-2021"));
        assert_eq!(h.acquired_time.as_deref(), Some("16:20:52"));
        assert_eq!(h.instrument.as_deref(), Some("QTOF"));
        // Empty "User Name" should not populate operator
        assert!(h.operator.is_none());
        assert_eq!(
            h.sample_description.as_deref(),
            Some("1,52 ul/ul in 200 mM AAc, Quad. 500")
        );
    }

    #[test]
    fn parse_5_coefficient_t1() {
        let h: Header = HEADER_PXD058812.parse().unwrap();
        let cal = h.cal_functions.get(&1).expect("Cal Function 1 missing");
        assert_eq!(cal.cal_type, CalType::T1);
        assert_eq!(cal.coeffs.len(), 5);
        assert!((cal.coeffs[0] - -3.072270525784614e-2).abs() < 1e-20);
        assert!((cal.coeffs[1] - 9.999211110337035e-1).abs() < 1e-15);
    }

    #[test]
    fn parse_6_coefficient_t1_three_functions() {
        let h: Header = HEADER_PXD075602.parse().unwrap();
        assert_eq!(h.cal_functions.len(), 3);
        for n in 1..=3u32 {
            let cal = h.cal_functions.get(&n).unwrap_or_else(|| panic!("Cal Function {n} missing"));
            assert_eq!(cal.cal_type, CalType::T1);
            assert_eq!(cal.coeffs.len(), 6);
            assert!((cal.coeffs[5] - 5.118758341877316e-11).abs() < 1e-25);
        }
    }

    #[test]
    fn t1_polynomial_apply_identity_coefficients() {
        // With c0=0, c1=1, all higher terms zero: t_cal should equal t_raw
        let cal = FunctionCal {
            coeffs: vec![0.0, 1.0],
            cal_type: CalType::T1,
        };
        let t = 42.5_f64;
        assert!((cal.apply(t) - t).abs() < 1e-12);
    }

    #[test]
    fn t0_apply_is_identity() {
        let cal = FunctionCal {
            coeffs: vec![],
            cal_type: CalType::T0,
        };
        assert_eq!(cal.apply(99.0), 99.0);
    }

    #[test]
    fn t0_from_header_parses_empty_coeffs() {
        let src = "$$ Cal MS1 Static: ,T0\r\n";
        // The MS1 Static key does not match "Cal Function N:", so no entry in cal_functions.
        // Verify the parser does not error on this line.
        let h: Header = src.parse().unwrap();
        assert!(h.cal_functions.is_empty());
    }
}
