// PyO3 bindings for the openwraw library.
//
// Exposes a high-level `RawReader` class that opens a Waters .raw directory
// and provides Python-friendly access to functions, spectra, and chromatograms.

use std::path::PathBuf;

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;

use ::openwraw::raw::{
    chroms::{read_chro_dat, ChromsInf},
    data::{decode_encoding_a, decode_encoding_b, decode_encoding_c, DecodeParams},
    extern_inf::{ExternInf, Polarity},
    functions_inf::FunctionTable,
    header::Header,
    index::ScanIndex,
};

// ── Error conversion ──────────────────────────────────────────────────────────

fn to_py_err(e: ::openwraw::Error) -> PyErr {
    PyRuntimeError::new_err(format!("{e}"))
}

fn io_to_py(e: std::io::Error) -> PyErr {
    PyRuntimeError::new_err(format!("{e}"))
}

// ── RunHeader ─────────────────────────────────────────────────────────────────

/// Acquisition metadata from `_HEADER.TXT`.
///
/// Returned by `RawReader.header`.
#[pyclass(from_py_object)]
#[derive(Clone)]
pub struct RunHeader {
    inner: Header,
}

#[pymethods]
impl RunHeader {
    /// MassLynx file format version string (e.g. `"01.00"`).
    #[getter]
    fn version(&self) -> Option<&str> {
        self.inner.version.as_deref()
    }

    /// Sample or acquisition file name recorded at collection time.
    #[getter]
    fn acquired_name(&self) -> Option<&str> {
        self.inner.acquired_name.as_deref()
    }

    /// Acquisition date string (e.g. `"14-Jan-2021"`).
    #[getter]
    fn acquired_date(&self) -> Option<&str> {
        self.inner.acquired_date.as_deref()
    }

    /// Acquisition time string (e.g. `"16:20:52"`).
    #[getter]
    fn acquired_time(&self) -> Option<&str> {
        self.inner.acquired_time.as_deref()
    }

    /// Instrument identifier string (e.g. `"QTOF"`, `"XEVO-G2XSQTOF#NotSet"`).
    #[getter]
    fn instrument(&self) -> Option<&str> {
        self.inner.instrument.as_deref()
    }

    /// Operator / user name recorded at collection time.
    #[getter]
    fn operator(&self) -> Option<&str> {
        self.inner.operator.as_deref()
    }

    /// Free-text sample description field.
    #[getter]
    fn sample_description(&self) -> Option<&str> {
        self.inner.sample_description.as_deref()
    }

    fn __repr__(&self) -> String {
        format!(
            "RunHeader(instrument={:?}, acquired_date={:?})",
            self.inner.instrument.as_deref().unwrap_or(""),
            self.inner.acquired_date.as_deref().unwrap_or(""),
        )
    }
}

// ── FunctionInfo ──────────────────────────────────────────────────────────────

/// Metadata for a single acquisition function from `_FUNCTNS.INF`.
#[pyclass(from_py_object)]
#[derive(Clone)]
pub struct FunctionInfo {
    inner: ::openwraw::raw::functions_inf::FunctionInfo,
}

#[pymethods]
impl FunctionInfo {
    /// 1-based function index.
    #[getter]
    fn index(&self) -> u32 {
        self.inner.index
    }

    /// Raw function type code (always 0x12 in known corpus).
    #[getter]
    fn function_type(&self) -> u8 {
        self.inner.function_type
    }

    /// Scan subtype byte; bit 7 set means lock-mass channel.
    #[getter]
    fn scan_subtype(&self) -> u8 {
        self.inner.scan_subtype
    }

    /// Total slot duration per scan cycle (seconds).
    #[getter]
    fn cycle_time_s(&self) -> f32 {
        self.inner.cycle_time_s
    }

    /// Idle time between end of one scan and start of the next (seconds).
    #[getter]
    fn interscan_delay_s(&self) -> f32 {
        self.inner.interscan_delay_s
    }

    /// Data collection time per scan (seconds).
    #[getter]
    fn scan_time_s(&self) -> f32 {
        self.inner.scan_time_s
    }

    /// Number of TDC bins per pusher pulse.
    #[getter]
    fn tof_depth(&self) -> u16 {
        self.inner.tof_depth
    }

    /// Acquisition m/z lower bound (Da).
    #[getter]
    fn mz_low(&self) -> f32 {
        self.inner.mz_low
    }

    /// Acquisition m/z upper bound (Da).
    #[getter]
    fn mz_high(&self) -> f32 {
        self.inner.mz_high
    }

    /// True if this is a lock-mass / reference channel (bit 7 of scan_subtype).
    #[getter]
    fn is_lock_mass(&self) -> bool {
        self.inner.is_lock_mass()
    }

    fn __repr__(&self) -> String {
        format!(
            "FunctionInfo(index={}, mz=[{:.0},{:.0}], scan_subtype={:#04x}, lock_mass={})",
            self.inner.index,
            self.inner.mz_low,
            self.inner.mz_high,
            self.inner.scan_subtype,
            self.inner.is_lock_mass(),
        )
    }
}

// ── Spectrum ──────────────────────────────────────────────────────────────────

/// A decoded 1-D mass spectrum (m/z vs intensity).
///
/// Returned by `RawReader.read_spectrum()` for Encoding A and C functions.
#[pyclass]
pub struct Spectrum {
    /// Calibrated m/z values (Da).
    #[pyo3(get)]
    pub mz: Vec<f64>,
    /// Intensity values.
    #[pyo3(get)]
    pub intensity: Vec<f32>,
}

#[pymethods]
impl Spectrum {
    fn __len__(&self) -> usize {
        self.mz.len()
    }

    fn __repr__(&self) -> String {
        format!("Spectrum({} peaks)", self.mz.len())
    }
}

// ── ImsSpectrum ───────────────────────────────────────────────────────────────

/// A decoded IMS spectrum: m/z, drift time, and intensity per ion.
///
/// Returned by `RawReader.read_ims_spectrum()` for Encoding B (SYNAPT) functions.
#[pyclass]
pub struct ImsSpectrum {
    /// Calibrated m/z values (Da).
    #[pyo3(get)]
    pub mz: Vec<f64>,
    /// Ion drift times (ms).
    #[pyo3(get)]
    pub drift_time_ms: Vec<f64>,
    /// Intensity values (raw ion counts).
    #[pyo3(get)]
    pub intensity: Vec<f32>,
}

#[pymethods]
impl ImsSpectrum {
    fn __len__(&self) -> usize {
        self.mz.len()
    }

    fn __repr__(&self) -> String {
        format!("ImsSpectrum({} ions)", self.mz.len())
    }
}

// ── ChromChannel ──────────────────────────────────────────────────────────────

/// Description of a single recorded chromatographic channel from `_CHROMS.INF`.
#[pyclass(from_py_object)]
#[derive(Clone)]
pub struct ChromChannel {
    /// 0-based index among data records.
    #[pyo3(get)]
    pub index: usize,
    /// Source device type (4 = BSM pump, 1 = column/sample device).
    #[pyo3(get)]
    pub source_type: u32,
    /// Channel name decoded from Windows-1252.
    #[pyo3(get)]
    pub name: String,
    /// Scale factor from the `$CC$` spec string.
    #[pyo3(get)]
    pub scale_f: f64,
    /// Engineering units (e.g. "%", "C", "bar").
    #[pyo3(get)]
    pub units: String,
}

#[pymethods]
impl ChromChannel {
    fn __repr__(&self) -> String {
        format!(
            "ChromChannel(index={}, name='{}', units='{}')",
            self.index, self.name, self.units
        )
    }
}

// ── ChromPoint ────────────────────────────────────────────────────────────────

/// A single (retention-time, value) sample from a `_CHROnnnn.DAT` file.
#[pyclass(from_py_object)]
#[derive(Clone)]
pub struct ChromPoint {
    /// Retention time (minutes).
    #[pyo3(get)]
    pub rt_min: f32,
    /// Channel value in the units given by `ChromChannel.units`.
    #[pyo3(get)]
    pub value: f32,
}

#[pymethods]
impl ChromPoint {
    fn __repr__(&self) -> String {
        format!(
            "ChromPoint(rt_min={:.4}, value={:.4})",
            self.rt_min, self.value
        )
    }
}

// ── RawReader ─────────────────────────────────────────────────────────────────

/// Open a Waters MassLynx `.raw` directory and read its contents.
///
/// Example::
///
///     import openwraw
///     r = openwraw.RawReader("/data/sample.raw")
///     print(r.functions)
///     spec = r.read_spectrum(1, 0)
///     print(spec.mz[:5], spec.intensity[:5])
#[pyclass]
pub struct RawReader {
    raw_dir: PathBuf,
    header: Header,
    ext: ExternInf,
    funcs: FunctionTable,
    chroms: Option<ChromsInf>,
}

#[pymethods]
impl RawReader {
    /// Open a .raw directory.
    ///
    /// Reads `_HEADER.TXT`, `_extern.inf`, `_FUNCTNS.INF`, and optionally
    /// `_CHROMS.INF` at construction time.  Spectrum data is read on demand.
    #[new]
    fn new(path: &str) -> PyResult<Self> {
        let raw_dir = PathBuf::from(path);
        if !raw_dir.is_dir() {
            return Err(PyRuntimeError::new_err(format!(
                "'{}' is not a directory",
                path
            )));
        }

        let header = Header::from_path(&raw_dir.join("_HEADER.TXT")).map_err(to_py_err)?;
        let ext = ExternInf::from_path(&raw_dir.join("_extern.inf")).map_err(to_py_err)?;
        let funcs = FunctionTable::from_path(&raw_dir.join("_FUNCTNS.INF")).map_err(to_py_err)?;

        let chroms_path = raw_dir.join("_CHROMS.INF");
        let chroms = if chroms_path.exists() {
            Some(ChromsInf::from_path(&chroms_path).map_err(to_py_err)?)
        } else {
            None
        };

        Ok(Self {
            raw_dir,
            header,
            ext,
            funcs,
            chroms,
        })
    }

    /// Acquisition metadata parsed from `_HEADER.TXT`.
    #[getter]
    fn header(&self) -> RunHeader {
        RunHeader {
            inner: self.header.clone(),
        }
    }

    /// Electrospray polarity parsed from `_extern.inf`.
    ///
    /// Returns `"positive"`, `"negative"`, or `None` when the field is absent.
    #[getter]
    fn polarity(&self) -> Option<&'static str> {
        self.ext.polarity.map(|p| match p {
            Polarity::Positive => "positive",
            Polarity::Negative => "negative",
        })
    }

    /// List of all acquisition functions in this .raw file.
    #[getter]
    fn functions(&self) -> Vec<FunctionInfo> {
        self.funcs
            .functions
            .iter()
            .map(|f| FunctionInfo { inner: f.clone() })
            .collect()
    }

    /// List of instrument chromatographic channels from `_CHROMS.INF`.
    ///
    /// Empty list if the file is absent.
    #[getter]
    fn channels(&self) -> Vec<ChromChannel> {
        match &self.chroms {
            None => vec![],
            Some(ci) => ci
                .channels
                .iter()
                .map(|ch| ChromChannel {
                    index: ch.index,
                    source_type: ch.source_type,
                    name: ch.name.clone(),
                    scale_f: ch.scale_f,
                    units: ch.units.clone(),
                })
                .collect(),
        }
    }

    /// MS level for a function (1-based `func_index`).
    ///
    /// Returns `1` for MS1 survey and reference functions, `2` for MSe/DDA/MS2
    /// functions. Falls back to `1` when the function is not described in
    /// `_extern.inf`.
    fn ms_level(&self, func_index: u32) -> u32 {
        self.ext
            .functions
            .get(&func_index)
            .map(|f| f.mode.ms_level())
            .unwrap_or(1)
    }

    /// Number of scans in a function (1-based `func_index`).
    fn n_scans(&self, func_index: u32) -> PyResult<usize> {
        let (idx, _dat) = self.load_idx_dat(func_index)?;
        Ok(idx.len())
    }

    /// Retention time (minutes) for a scan.
    ///
    /// `func_index` is 1-based; `scan_index` is 0-based.
    fn retention_time(&self, func_index: u32, scan_index: usize) -> PyResult<f32> {
        let (idx, _dat) = self.load_idx_dat(func_index)?;
        match idx {
            ScanIndex::A(scans) => scans
                .get(scan_index)
                .map(|s| s.retention_time_min)
                .ok_or_else(|| PyRuntimeError::new_err(format!("scan {scan_index} out of range"))),
            ScanIndex::B(scans) => scans
                .get(scan_index)
                .map(|s| s.retention_time_min)
                .ok_or_else(|| PyRuntimeError::new_err(format!("scan {scan_index} out of range"))),
        }
    }

    /// Decode a 1-D mass spectrum.
    ///
    /// Uses Encoding A for older Q-TOF functions and Encoding C for G2/G2-Si.
    /// For IMS data, this collapses the drift dimension; use `read_ims_spectrum`
    /// to obtain the full 2-D data.
    ///
    /// `func_index` is 1-based; `scan_index` is 0-based.
    fn read_spectrum(&self, func_index: u32, scan_index: usize) -> PyResult<Spectrum> {
        let f = self.get_function(func_index)?;
        let params = self.make_params(&f, func_index);
        let (idx, dat) = self.load_idx_dat(func_index)?;

        match idx {
            ScanIndex::A(scans) => {
                let scan = scans.get(scan_index).ok_or_else(|| {
                    PyRuntimeError::new_err(format!("scan {scan_index} out of range"))
                })?;
                let start = scan.dat_offset as usize;
                let end = start + scan.n_records as usize * 6;
                if end > dat.len() {
                    return Err(PyRuntimeError::new_err("scan offset out of DAT bounds"));
                }
                let spec = decode_encoding_a(&dat[start..end], &params).map_err(to_py_err)?;
                Ok(Spectrum {
                    mz: spec.mz,
                    intensity: spec.intensity,
                })
            }
            ScanIndex::B(scans) => {
                let scan = scans.get(scan_index).ok_or_else(|| {
                    PyRuntimeError::new_err(format!("scan {scan_index} out of range"))
                })?;
                let start = scan.dat_offset as usize;
                let end = scans
                    .get(scan_index + 1)
                    .map(|s| s.dat_offset as usize)
                    .unwrap_or(dat.len());
                if end <= start || end > dat.len() {
                    return Ok(Spectrum {
                        mz: vec![],
                        intensity: vec![],
                    });
                }
                let spec = decode_encoding_c(&dat[start..end], &params).map_err(to_py_err)?;
                Ok(Spectrum {
                    mz: spec.mz,
                    intensity: spec.intensity,
                })
            }
        }
    }

    /// Decode a full IMS spectrum (m/z, drift time, intensity) for SYNAPT data.
    ///
    /// Only valid for Encoding B functions (IDX Variant B).  Returns a
    /// `RuntimeError` if the function uses Encoding A.
    ///
    /// `func_index` is 1-based; `scan_index` is 0-based.
    fn read_ims_spectrum(&self, func_index: u32, scan_index: usize) -> PyResult<ImsSpectrum> {
        let f = self.get_function(func_index)?;
        let params = self.make_params(&f, func_index);
        let (idx, dat) = self.load_idx_dat(func_index)?;

        match idx {
            ScanIndex::A(_) => Err(PyRuntimeError::new_err(
                "function uses Encoding A (not IMS); use read_spectrum instead",
            )),
            ScanIndex::B(scans) => {
                let scan = scans.get(scan_index).ok_or_else(|| {
                    PyRuntimeError::new_err(format!("scan {scan_index} out of range"))
                })?;
                let start = scan.dat_offset as usize;
                let end = scans
                    .get(scan_index + 1)
                    .map(|s| s.dat_offset as usize)
                    .unwrap_or(dat.len());
                if end <= start || end > dat.len() {
                    return Ok(ImsSpectrum {
                        mz: vec![],
                        drift_time_ms: vec![],
                        intensity: vec![],
                    });
                }
                let spec = decode_encoding_b(&dat[start..end], &params).map_err(to_py_err)?;
                Ok(ImsSpectrum {
                    mz: spec.mz,
                    drift_time_ms: spec.drift_time_ms,
                    intensity: spec.intensity,
                })
            }
        }
    }

    /// Read a chromatographic channel as a list of `ChromPoint` values.
    ///
    /// `channel_index` is 0-based (matches `ChromChannel.index`).
    fn read_chrom(&self, channel_index: usize) -> PyResult<Vec<ChromPoint>> {
        let ci = self
            .chroms
            .as_ref()
            .ok_or_else(|| PyRuntimeError::new_err("no _CHROMS.INF in this .raw directory"))?;
        let chro_num = ci.chro_number_for_channel(channel_index);
        let chro_path = self.raw_dir.join(format!("_CHRO{chro_num:03}.DAT"));
        let points = read_chro_dat(&chro_path).map_err(to_py_err)?;
        Ok(points
            .iter()
            .map(|p| ChromPoint {
                rt_min: p.rt_min,
                value: p.value,
            })
            .collect())
    }

    fn __repr__(&self) -> String {
        format!(
            "RawReader('{}', {} function(s), {} channel(s))",
            self.raw_dir.display(),
            self.funcs.functions.len(),
            self.chroms.as_ref().map(|c| c.channels.len()).unwrap_or(0),
        )
    }
}

impl RawReader {
    fn get_function(
        &self,
        func_index: u32,
    ) -> PyResult<::openwraw::raw::functions_inf::FunctionInfo> {
        self.funcs
            .functions
            .iter()
            .find(|f| f.index == func_index)
            .cloned()
            .ok_or_else(|| PyRuntimeError::new_err(format!("function {func_index} not found")))
    }

    fn make_params(
        &self,
        f: &::openwraw::raw::functions_inf::FunctionInfo,
        func_index: u32,
    ) -> DecodeParams {
        DecodeParams {
            a_us: self.ext.a_us(),
            cal: self
                .header
                .cal_functions
                .get(&func_index)
                .cloned()
                .unwrap_or_default(),
            mz_low: f.mz_low as f64,
            mz_high: f.mz_high as f64,
            scan_time_ms: f.scan_time_s as f64 * 1000.0,
        }
    }

    fn load_idx_dat(&self, func_index: u32) -> PyResult<(ScanIndex, Vec<u8>)> {
        let idx_path = self.raw_dir.join(format!("_FUNC{func_index:03}.IDX"));
        let dat_path = self.raw_dir.join(format!("_FUNC{func_index:03}.DAT"));

        if !idx_path.exists() {
            return Err(PyRuntimeError::new_err(format!(
                "IDX file not found: {}",
                idx_path.display()
            )));
        }
        if !dat_path.exists() {
            return Err(PyRuntimeError::new_err(format!(
                "DAT file not found: {}",
                dat_path.display()
            )));
        }

        let idx_bytes = std::fs::read(&idx_path).map_err(io_to_py)?;
        let dat = std::fs::read(&dat_path).map_err(io_to_py)?;
        let idx = ScanIndex::from_bytes(&idx_bytes).map_err(to_py_err)?;
        Ok((idx, dat))
    }
}

// ── Module ────────────────────────────────────────────────────────────────────

#[pymodule]
fn openwraw(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<RawReader>()?;
    m.add_class::<RunHeader>()?;
    m.add_class::<FunctionInfo>()?;
    m.add_class::<Spectrum>()?;
    m.add_class::<ImsSpectrum>()?;
    m.add_class::<ChromChannel>()?;
    m.add_class::<ChromPoint>()?;
    Ok(())
}
