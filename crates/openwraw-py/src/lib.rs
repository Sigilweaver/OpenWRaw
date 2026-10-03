#![cfg_attr(not(test), warn(clippy::unwrap_used, clippy::expect_used))]
// PyO3 bindings for the openwraw library.
//
// Exposes a high-level `RawReader` class that opens a Waters .raw directory
// and provides Python-friendly access to functions, spectra, and chromatograms.

use std::path::{Path, PathBuf};
use std::sync::{
    mpsc::{sync_channel, Receiver},
    Mutex,
};

use openmassspec_core::{SpectrumRecord, SpectrumSource};
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::PyDict;

use ::openwraw::raw::{
    chroms::{read_chro_dat, ChromsInf},
    extern_inf::{ExternInf, Polarity},
    functions_inf::FunctionTable,
    header::Header,
    index::ScanIndex,
};
use ::openwraw::{DecodedSpectrum, Encoding, Reader};

// -- Error conversion --

fn to_py_err(e: ::openwraw::Error) -> PyErr {
    PyRuntimeError::new_err(format!("{e}"))
}

/// Spectrum record as a dict. `total_ion_current` and `base_peak_*` carry the
/// effective values, matching `openmassspec_io.Spectrum`; the vendor-reported
/// values stay under `reported_*`. Peak arrays bypass JSON, which would turn
/// NaN into `None` and is slow for large spectra.
fn record_object(py: Python<'_>, mut rec: SpectrumRecord) -> PyResult<Py<PyAny>> {
    let tic = rec.effective_tic();
    let base_peak = rec.effective_base_peak();
    let mz = std::mem::take(&mut rec.mz);
    let intensity = std::mem::take(&mut rec.intensity);
    let mobility = rec.inv_mobility_per_peak.take();
    let obj = json_object(py, &rec)?;
    let d = obj.bind(py);
    d.set_item("mz", mz)?;
    d.set_item("intensity", intensity)?;
    d.set_item("inv_mobility_per_peak", mobility)?;
    d.set_item("total_ion_current", tic)?;
    d.set_item("base_peak_mz", base_peak.map(|p| p.0))?;
    d.set_item("base_peak_intensity", base_peak.map(|p| p.1))?;
    Ok(obj)
}

fn json_object<T: serde::Serialize>(py: Python<'_>, value: &T) -> PyResult<Py<PyAny>> {
    let mut value =
        serde_json::to_value(value).map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
    normalize_record_json(&mut value);
    let json = serde_json::to_string(&value).map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
    Ok(py.import("json")?.call_method1("loads", (json,))?.unbind())
}

fn normalize_record_json(value: &mut serde_json::Value) {
    if let serde_json::Value::Object(fields) = value {
        if fields.contains_key("native_id") || fields.contains_key("source_file_name") {
            fields
                .entry("extra")
                .or_insert_with(|| serde_json::json!({}));
        }
        if fields.contains_key("native_id") {
            for (source, alias) in [
                ("total_ion_current", "reported_total_ion_current"),
                ("base_peak_mz", "reported_base_peak_mz"),
                ("base_peak_intensity", "reported_base_peak_intensity"),
            ] {
                if let Some(reported) = fields.get(source).cloned() {
                    fields.insert(alias.to_string(), reported);
                }
            }
        }
        for (key, field) in fields {
            match key.as_str() {
                "polarity" | "scan_mode" | "analyzer" | "activation" => {
                    if let Some(text) = field.as_str() {
                        *field = serde_json::Value::String(text.to_ascii_lowercase());
                    }
                }
                "mobility_array_kind" => {
                    if let Some(text) = field.as_str() {
                        let normalized = match text {
                            "InverseReducedVsPerCm2" => "inverse_reduced_k0",
                            "DriftTimeMilliseconds" => "drift_time_ms",
                            other => other,
                        };
                        *field = serde_json::Value::String(normalized.to_string());
                    }
                }
                "analyzers" => {
                    if let Some(items) = field.as_array_mut() {
                        for item in items {
                            if let Some(text) = item.as_str() {
                                *item = serde_json::Value::String(text.to_ascii_lowercase());
                            }
                        }
                    }
                }
                _ => normalize_record_json(field),
            }
        }
    }
}

/// Bounded stream of canonical spectrum records.
#[pyclass(module = "openwraw")]
struct RecordIter {
    // `None` marks a complete stream, so a closed channel without it means
    // the decode thread died.
    receiver: Mutex<Receiver<Option<SpectrumRecord>>>,
    finished: bool,
}

#[pymethods]
impl RecordIter {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(mut slf: PyRefMut<'_, Self>, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        if slf.finished {
            return Ok(None);
        }
        let receiver = &slf.receiver;
        let message = py.detach(|| {
            receiver
                .lock()
                .map_err(|_| "record stream lock poisoned")?
                .recv()
                .map_err(|_| "record decode thread ended without a result")
        });
        match message {
            Ok(Some(rec)) => record_object(py, rec).map(Some),
            Ok(None) => {
                slf.finished = true;
                Ok(None)
            }
            Err(error) => {
                slf.finished = true;
                Err(PyRuntimeError::new_err(error))
            }
        }
    }
}

fn find_side_file(dir: &Path, name: &str) -> ::openwraw::Result<Option<PathBuf>> {
    let wanted = name.to_ascii_uppercase();
    let mut suffix = None;
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let actual = entry.file_name().to_string_lossy().to_ascii_uppercase();
        if actual == wanted {
            return Ok(Some(entry.path()));
        }
        if actual.ends_with(&wanted) {
            if suffix.is_some() {
                return Err(::openwraw::Error::Parse(format!(
                    "multiple files match {name} in {}",
                    dir.display()
                )));
            }
            suffix = Some(entry.path());
        }
    }
    Ok(suffix)
}

// -- RunHeader --

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
    /// Per-function mass calibration polynomials from `_HEADER.TXT`.
    #[getter]
    fn calibrations<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let out = PyDict::new(py);
        for (index, cal) in &self.inner.cal_functions {
            let row = PyDict::new(py);
            row.set_item("coeffs", &cal.coeffs)?;
            row.set_item(
                "cal_type",
                format!("{:?}", cal.cal_type).to_ascii_lowercase(),
            )?;
            out.set_item(index, row)?;
        }
        Ok(out)
    }
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

// -- FunctionInfo --

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

// -- Spectrum --

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

// -- ImsSpectrum --

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

// -- ChromChannel --

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

// -- ChromPoint --

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

// -- RawReader --

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
    reader: Reader,
    header: Header,
    ext: ExternInf,
    funcs: FunctionTable,
    chroms: Option<ChromsInf>,
}

#[pymethods]
impl RawReader {
    /// Stream full canonical spectrum records with a two-record buffer.
    fn iter_records(&self) -> RecordIter {
        let mut source = ::openwraw::mzml::WatersSource::new(self.reader.clone());
        let (sender, receiver) = sync_channel(2);
        std::thread::spawn(move || {
            for record in source.iter_spectra() {
                if sender.send(Some(record)).is_err() {
                    return;
                }
            }
            let _ = sender.send(None);
        });
        RecordIter {
            receiver: Mutex::new(receiver),
            finished: false,
        }
    }

    /// Canonical run metadata used by the Rust mzML writer.
    fn run_info(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        json_object(
            py,
            &::openwraw::mzml::WatersSource::new(self.reader.clone()).run_metadata(),
        )
    }

    /// Canonical TIC, BPC, and other chromatograms with seconds as the time unit.
    fn read_chromatograms(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        let mut source = ::openwraw::mzml::WatersSource::new(self.reader.clone());
        source
            .iter_chromatograms()
            .map(|rec| json_object(py, &rec))
            .collect()
    }

    /// Full canonical record for one scan in the non-lock-mass spectrum stream.
    fn read_record(
        &self,
        py: Python<'_>,
        func_index: u32,
        scan_index: usize,
    ) -> PyResult<Py<PyAny>> {
        let function = self.function(func_index)?;
        if function.info.is_lock_mass() || scan_index >= function.scan_count() {
            return Err(PyRuntimeError::new_err(
                "scan is outside the canonical spectrum stream",
            ));
        }
        let before: usize = self
            .reader
            .functions
            .iter()
            .filter(|f| !f.info.is_lock_mass() && f.index < func_index)
            .map(|f| f.scan_count())
            .sum();
        let counter = u32::try_from(before + scan_index + 1)
            .map_err(|_| PyRuntimeError::new_err("scan counter exceeds u32"))?;
        let scan = self
            .reader
            .decode_scan(func_index, scan_index)
            .map_err(to_py_err)?;
        let record = ::openwraw::mzml::record_from_scan(&self.reader, counter, scan);
        record_object(py, record)
    }

    /// Raw instrument values decoded from `_extern.inf`.
    fn instrument_parameters<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let out = PyDict::new(py);
        out.set_item("lteff_mm", self.ext.lteff_mm)?;
        out.set_item("veff_v", self.ext.veff_v)?;
        out.set_item("pusher_interval_us", self.ext.pusher_interval_us)?;
        out.set_item("polarity", self.polarity())?;
        let functions = PyDict::new(py);
        for (index, f) in &self.ext.functions {
            let row = PyDict::new(py);
            row.set_item("index", f.index)?;
            row.set_item("pusher_interval_us", f.pusher_interval_us)?;
            row.set_item("mode", format!("{:?}", f.mode))?;
            row.set_item("set_mass_da", f.set_mass_da)?;
            functions.set_item(index, row)?;
        }
        out.set_item("functions", functions)?;
        Ok(out)
    }

    /// Collision energy and ETD mode decoded for a scan from `_FUNCnnn.STS`.
    fn scan_parameters<'py>(
        &self,
        py: Python<'py>,
        func_index: u32,
        scan_index: usize,
    ) -> PyResult<Bound<'py, PyDict>> {
        let scan = self
            .reader
            .decode_scan(func_index, scan_index)
            .map_err(to_py_err)?;
        let out = PyDict::new(py);
        out.set_item("collision_energy_ev", scan.collision_energy_ev)?;
        out.set_item("etd_fragmentation_mode", scan.etd_fragmentation_mode)?;
        Ok(out)
    }

    /// Every named statistics channel decoded for a scan from `_FUNCnnn.STS`.
    fn scan_channels<'py>(
        &self,
        py: Python<'py>,
        func_index: u32,
        scan_index: usize,
    ) -> PyResult<Bound<'py, PyDict>> {
        let function = self.function(func_index)?;
        if scan_index >= function.scan_count() {
            return Err(PyRuntimeError::new_err(format!(
                "scan {scan_index} out of range"
            )));
        }
        let out = PyDict::new(py);
        if let Some(sts) = &function.sts {
            for channel in sts.channels() {
                out.set_item(&channel.name, sts.value_at(channel, scan_index))?;
            }
        }
        Ok(out)
    }

    /// Source sequence, encoding, and byte offset of every statistics channel.
    fn channel_descriptors<'py>(
        &self,
        py: Python<'py>,
        func_index: u32,
    ) -> PyResult<Vec<Bound<'py, PyDict>>> {
        let function = self.function(func_index)?;
        let mut out = Vec::new();
        if let Some(sts) = &function.sts {
            for channel in sts.channels() {
                let row = PyDict::new(py);
                row.set_item("seq", channel.seq)?;
                row.set_item("name", &channel.name)?;
                row.set_item(
                    "encoding",
                    format!("{:?}", channel.encoding).to_ascii_lowercase(),
                )?;
                row.set_item("offset", channel.offset)?;
                out.push(row);
            }
        }
        Ok(out)
    }

    /// One decoded `_FUNCnnn.IDX` record, including vendor record offsets.
    fn index_record<'py>(
        &self,
        py: Python<'py>,
        func_index: u32,
        scan_index: usize,
    ) -> PyResult<Bound<'py, PyDict>> {
        let function = self.function(func_index)?;
        let out = PyDict::new(py);
        match &function.scan_index {
            ScanIndex::A(records) => {
                let row = records.get(scan_index).ok_or_else(|| {
                    PyRuntimeError::new_err(format!("scan {scan_index} out of range"))
                })?;
                out.set_item("variant", "a")?;
                out.set_item("dat_offset", row.dat_offset)?;
                out.set_item("n_records", row.n_records)?;
                out.set_item("retention_time_min", row.retention_time_min)?;
                out.set_item("peak_count", row.peak_count)?;
            }
            ScanIndex::B(records) => {
                let row = records.get(scan_index).ok_or_else(|| {
                    PyRuntimeError::new_err(format!("scan {scan_index} out of range"))
                })?;
                out.set_item("variant", "b")?;
                out.set_item("dat_offset", row.dat_offset)?;
                out.set_item("retention_time_min", row.retention_time_min)?;
            }
        }
        Ok(out)
    }
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

        let reader = Reader::open(&raw_dir).map_err(to_py_err)?;
        let header = reader.header.clone();
        let ext = reader.extern_inf.clone();
        let funcs_path = find_side_file(&raw_dir, "_FUNCTNS.INF")
            .map_err(to_py_err)?
            .ok_or_else(|| PyRuntimeError::new_err("_FUNCTNS.INF not found"))?;
        let funcs = FunctionTable::from_path(&funcs_path).map_err(to_py_err)?;

        let chroms_path = find_side_file(&raw_dir, "_CHROMS.INF").map_err(to_py_err)?;
        let chroms = if let Some(chroms_path) = chroms_path {
            Some(ChromsInf::from_path(&chroms_path).map_err(|e| {
                PyRuntimeError::new_err(format!("reading {}: {e}", chroms_path.display()))
            })?)
        } else {
            None
        };

        Ok(Self {
            raw_dir,
            reader,
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
        Ok(self.function(func_index)?.scan_count())
    }

    /// Encoding variant for a function (1-based `func_index`).
    ///
    /// Returns `"a"`, `"c"` or `"d"` for one-dimensional spectra, `"b"` for IMS.
    fn function_encoding(&self, func_index: u32) -> PyResult<&'static str> {
        Ok(match self.function(func_index)?.encoding {
            Encoding::A => "a",
            Encoding::B => "b",
            Encoding::C => "c",
            Encoding::D => "d",
            Encoding::E => "e",
        })
    }

    /// Retention time (minutes) for a scan.
    ///
    /// `func_index` is 1-based; `scan_index` is 0-based.
    fn retention_time(&self, func_index: u32, scan_index: usize) -> PyResult<f32> {
        match &self.function(func_index)?.scan_index {
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
        let scan = self
            .reader
            .decode_scan(func_index, scan_index)
            .map_err(to_py_err)?;
        match scan.spectrum {
            DecodedSpectrum::Plain(spec) => Ok(Spectrum {
                mz: spec.mz,
                intensity: spec.intensity,
            }),
            DecodedSpectrum::Ims(spec) => Ok(Spectrum {
                mz: spec.mz,
                intensity: spec.intensity,
            }),
        }
    }

    /// Decode a full IMS spectrum (m/z, drift time, intensity) for SYNAPT data.
    ///
    /// Only valid for Encoding B functions. Returns a `RuntimeError` for
    /// one-dimensional functions.
    ///
    /// `func_index` is 1-based; `scan_index` is 0-based.
    fn read_ims_spectrum(&self, func_index: u32, scan_index: usize) -> PyResult<ImsSpectrum> {
        let scan = self
            .reader
            .decode_scan(func_index, scan_index)
            .map_err(to_py_err)?;
        match scan.spectrum {
            DecodedSpectrum::Ims(spec) => Ok(ImsSpectrum {
                mz: spec.mz,
                drift_time_ms: spec.drift_time_ms,
                intensity: spec.intensity,
            }),
            DecodedSpectrum::Plain(_) => Err(PyRuntimeError::new_err(
                "function is not IMS; use read_spectrum instead",
            )),
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
        let chro_path = find_side_file(&self.raw_dir, &format!("_CHRO{chro_num:03}.DAT"))
            .map_err(to_py_err)?
            .ok_or_else(|| PyRuntimeError::new_err(format!("CHRO file {chro_num} not found")))?;
        let points = read_chro_dat(&chro_path).map_err(|e| {
            PyRuntimeError::new_err(format!("reading {}: {e}", chro_path.display()))
        })?;
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
    fn function(&self, func_index: u32) -> PyResult<&::openwraw::FunctionEntry> {
        self.reader
            .functions
            .iter()
            .find(|f| f.index == func_index)
            .ok_or_else(|| PyRuntimeError::new_err(format!("function {func_index} not found")))
    }
}

// -- Module --

#[pymodule]
fn openwraw(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<RecordIter>()?;
    // Forward Rust `log` records to Python `logging` under the "openwraw"
    // logger hierarchy. Records below the configured Python level are
    // dropped cheaply; configure logging before opening files, since levels
    // are cached per logger.
    pyo3_log::init();
    m.add_class::<RawReader>()?;
    m.add_class::<RunHeader>()?;
    m.add_class::<FunctionInfo>()?;
    m.add_class::<Spectrum>()?;
    m.add_class::<ImsSpectrum>()?;
    m.add_class::<ChromChannel>()?;
    m.add_class::<ChromPoint>()?;
    Ok(())
}
