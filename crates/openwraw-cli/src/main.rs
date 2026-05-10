// openwraw-cli: command-line tool for reading Waters MassLynx RAW files.
//
// Usage:
//   openwraw inspect <raw-dir>
//   openwraw convert <raw-dir> [-o <output.mzML>] [--function N]

use std::io::{self, Write};
use std::path::{Path, PathBuf};

use openwraw::raw::{
    chroms::ChromsInf,
    data::{decode_encoding_a, decode_encoding_c, DecodeParams, Spectrum},
    extern_inf::ExternInf,
    functions_inf::{subtype, FunctionTable},
    header::Header,
    index::ScanIndex,
};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage:");
        eprintln!("  openwraw inspect <raw-dir>");
        eprintln!("  openwraw convert <raw-dir> [-o <output.mzML>] [--function N]");
        std::process::exit(1);
    }

    let raw = PathBuf::from(&args[2]);
    if !raw.exists() || !raw.is_dir() {
        eprintln!("error: '{}' is not a directory", raw.display());
        std::process::exit(1);
    }

    match args[1].as_str() {
        "inspect" => {
            if let Err(e) = cmd_inspect(&raw) {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        }
        "convert" => {
            let output = find_flag(&args, "-o").map(PathBuf::from);
            let func_filter = find_flag(&args, "--function")
                .and_then(|s| s.parse::<u32>().ok());
            if let Err(e) = cmd_convert(&raw, output.as_deref(), func_filter) {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        }
        cmd => {
            eprintln!("error: unknown command '{cmd}'");
            std::process::exit(1);
        }
    }
}

fn find_flag<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.windows(2)
        .find(|w| w[0] == flag)
        .map(|w| w[1].as_str())
}

// ── inspect ───────────────────────────────────────────────────────────────────

fn cmd_inspect(raw: &Path) -> openwraw::Result<()> {
    let header = Header::from_path(&raw.join("_HEADER.TXT"))?;
    let ext = ExternInf::from_path(&raw.join("_extern.inf"))?;
    let funcs = FunctionTable::from_path(&raw.join("_FUNCTNS.INF"))?;

    println!("=== _HEADER.TXT ===");
    println!("  Version:     {}", header.version.as_deref().unwrap_or("-"));
    println!("  Instrument:  {}", header.instrument.as_deref().unwrap_or("-"));
    println!("  Acquired:    {} {}",
        header.acquired_date.as_deref().unwrap_or("-"),
        header.acquired_time.as_deref().unwrap_or(""));
    println!("  Operator:    {}", header.operator.as_deref().unwrap_or("-"));
    if let Some(sd) = &header.sample_description {
        if !sd.is_empty() { println!("  Sample:      {sd}"); }
    }
    println!("  A_us:        {:.6} µs/sqrt(Da)", ext.a_us());

    println!("\n=== _FUNCTNS.INF ({} function(s)) ===", funcs.functions.len());
    for f in &funcs.functions {
        let enc = encoding_label(f.scan_subtype);
        let lock = if f.is_lock_mass() { " [lock mass]" } else { "" };
        println!(
            "  Function {}: type={:#04x} sub={:#04x}{} enc={} mz=[{:.0},{:.0}] scan={:.3}s",
            f.index,
            f.function_type,
            f.scan_subtype,
            lock,
            enc,
            f.mz_low,
            f.mz_high,
            f.scan_time_s
        );

        let idx_path = raw.join(format!("_FUNC{:03}.IDX", f.index));
        if idx_path.exists() {
            let idx_bytes = std::fs::read(&idx_path)?;
            match ScanIndex::from_bytes(&idx_bytes) {
                Ok(ScanIndex::A(v)) => println!(
                    "    IDX: Variant A, {} scans (Encoding A)",
                    v.len()
                ),
                Ok(ScanIndex::B(v)) => println!(
                    "    IDX: Variant B, {} scans (Encoding B/C)",
                    v.len()
                ),
                Err(e) => println!("    IDX: error ({e})"),
            }
        }
    }

    let cal_fns: Vec<_> = header.cal_functions.keys().collect();
    if !cal_fns.is_empty() {
        println!("\n  Calibrated functions: {:?}", cal_fns);
    }

    let chroms_path = raw.join("_CHROMS.INF");
    if chroms_path.exists() {
        let ci = ChromsInf::from_path(&chroms_path)?;
        println!("\n=== _CHROMS.INF ({} channel(s)) ===", ci.channels.len());
        for ch in &ci.channels {
            println!(
                "  Channel {}: [{}] {} (scale={}, units={})",
                ch.index + 1,
                ch.source_type,
                ch.name,
                ch.scale_f,
                ch.units
            );
        }
    }

    Ok(())
}

fn encoding_label(scan_subtype: u8) -> &'static str {
    if scan_subtype & 0x80 != 0 {
        "lock"
    } else if scan_subtype == subtype::OLDER_QTOF_SURVEY {
        "A"
    } else {
        "B/C"
    }
}

// ── convert ───────────────────────────────────────────────────────────────────

fn cmd_convert(
    raw: &Path,
    output: Option<&Path>,
    func_filter: Option<u32>,
) -> openwraw::Result<()> {
    let header = Header::from_path(&raw.join("_HEADER.TXT"))?;
    let ext = ExternInf::from_path(&raw.join("_extern.inf"))?;
    let funcs = FunctionTable::from_path(&raw.join("_FUNCTNS.INF"))?;

    // Collect functions to convert.
    let targets: Vec<_> = funcs
        .functions
        .iter()
        .filter(|f| {
            !f.is_lock_mass()
                && func_filter.map_or(true, |n| n == f.index)
        })
        .collect();

    if targets.is_empty() {
        eprintln!("warning: no functions to convert");
        return Ok(());
    }

    // Count total spectra for the mzML header.
    let mut total_spectra: usize = 0;
    let mut function_scans: Vec<(u32, ScanIndex)> = Vec::new();

    for f in &targets {
        let idx_path = raw.join(format!("_FUNC{:03}.IDX", f.index));
        if !idx_path.exists() {
            continue;
        }
        let idx_bytes = std::fs::read(&idx_path)?;
        let idx = ScanIndex::from_bytes(&idx_bytes)?;
        total_spectra += idx.len();
        function_scans.push((f.index, idx));
    }

    // Open output.
    let mut buf: Box<dyn io::Write> = match output {
        Some(path) => Box::new(io::BufWriter::new(
            std::fs::File::create(path).map_err(openwraw::Error::Io)?,
        )),
        None => Box::new(io::BufWriter::new(io::stdout())),
    };

    // Write mzML.
    write_mzml_header(
        &mut buf,
        total_spectra,
        raw.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown.raw"),
        &header,
        &ext,
    )?;

    let mut spectrum_index: usize = 0;
    for (func_index, idx) in &function_scans {
        let f = &funcs.functions[(*func_index as usize) - 1]; // 1-based to 0-based
        let dat_path = raw.join(format!("_FUNC{:03}.DAT", f.index));
        if !dat_path.exists() {
            continue;
        }
        let dat = std::fs::read(&dat_path)?;

        let cal = header
            .cal_functions
            .get(func_index)
            .cloned()
            .unwrap_or_default();

        let params = DecodeParams {
            a_us: ext.a_us(),
            cal,
            mz_low: f.mz_low as f64,
            mz_high: f.mz_high as f64,
            scan_time_ms: f.scan_time_s as f64 * 1000.0,
        };

        let use_encoding_a = f.scan_subtype == subtype::OLDER_QTOF_SURVEY;

        match idx {
            ScanIndex::A(scans) => {
                for (i, scan) in scans.iter().enumerate() {
                    let start = scan.dat_offset as usize;
                    let end = start + scan.n_records as usize * 6;
                    if end > dat.len() {
                        continue;
                    }
                    let spec = decode_encoding_a(&dat[start..end], &params)
                        .unwrap_or_default();
                    write_spectrum(
                        &mut buf,
                        spectrum_index,
                        i + 1,
                        scan.retention_time_min,
                        &spec,
                    )?;
                    spectrum_index += 1;
                }
            }
            ScanIndex::B(scans) => {
                for (i, scan) in scans.iter().enumerate() {
                    let start = scan.dat_offset as usize;
                    let end = scans
                        .get(i + 1)
                        .map(|s| s.dat_offset as usize)
                        .unwrap_or(dat.len());
                    if end <= start || end > dat.len() {
                        continue;
                    }
                    let spec = if use_encoding_a {
                        // Should not happen for Variant B IDX, but handle gracefully.
                        Spectrum::default()
                    } else {
                        // Use Encoding C for all Variant B functions (correct for
                        // non-IMS G2; approximation for IMS where sub_bin = dt_bin).
                        decode_encoding_c(&dat[start..end], &params).unwrap_or_default()
                    };
                    write_spectrum(
                        &mut buf,
                        spectrum_index,
                        i + 1,
                        scan.retention_time_min,
                        &spec,
                    )?;
                    spectrum_index += 1;
                }
            }
        }
    }

    write_mzml_footer(&mut buf)?;
    Ok(())
}

// ── mzML writer ───────────────────────────────────────────────────────────────

fn write_mzml_header(
    w: &mut impl Write,
    n_spectra: usize,
    raw_name: &str,
    header: &Header,
    _ext: &ExternInf,
) -> io::Result<()> {
    writeln!(w, r#"<?xml version="1.0" encoding="utf-8"?>"#)?;
    writeln!(w, r#"<mzML xmlns="http://psi.hupo.org/ms/mzml""#)?;
    writeln!(w, r#"      xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance""#)?;
    writeln!(w, r#"      xsi:schemaLocation="http://psi.hupo.org/ms/mzml http://psidev.info/files/ms/mzML/xsd/mzML1.1.0.xsd">"#)?;
    writeln!(
        w,
        r#"  <cvList count="2">
    <cv id="MS" fullName="Proteomics Standards Initiative Mass Spectrometry Ontology"
        version="4.1.30" URI="https://raw.githubusercontent.com/HUPO-PSI/psi-ms-CV/master/psi-ms.obo"/>
    <cv id="UO" fullName="Unit Ontology"
        version="09:04:2014" URI="https://raw.githubusercontent.com/bio-ontology-research-group/unit-ontology/master/unit.obo"/>
  </cvList>"#
    )?;

    writeln!(
        w,
        r#"  <fileDescription>
    <fileContent>
      <cvParam cvRef="MS" accession="MS:1000579" name="MS1 spectrum" value=""/>
    </fileContent>
    <sourceFileList count="1">
      <sourceFile id="sf1" name="{raw_name}" location="">
        <cvParam cvRef="MS" accession="MS:1000564" name="PSI mzData file" value=""/>
        <cvParam cvRef="MS" accession="MS:1000776" name="scan number only nativeID format" value=""/>
      </sourceFile>
    </sourceFileList>
  </fileDescription>"#
    )?;

    writeln!(
        w,
        r#"  <softwareList count="1">
    <software id="openwraw" version="0.1.0">
      <cvParam cvRef="MS" accession="MS:1000799" name="custom unreleased software tool" value="openwraw"/>
    </software>
  </softwareList>"#
    )?;

    let inst = xml_escape(header.instrument.as_deref().unwrap_or("-"));
    writeln!(
        w,
        r#"  <instrumentConfigurationList count="1">
    <instrumentConfiguration id="IC1">
      <cvParam cvRef="MS" accession="MS:1000031" name="instrument model" value="{inst}"/>
    </instrumentConfiguration>
  </instrumentConfigurationList>"#
    )?;

    writeln!(
        w,
        r#"  <dataProcessingList count="1">
    <dataProcessing id="openwraw_conversion">
      <processingMethod order="0" softwareRef="openwraw">
        <cvParam cvRef="MS" accession="MS:1000544" name="Conversion to mzML" value=""/>
      </processingMethod>
    </dataProcessing>
  </dataProcessingList>"#
    )?;

    writeln!(
        w,
        r#"  <run id="run1">
    <spectrumList count="{n_spectra}" defaultDataProcessingRef="openwraw_conversion">"#
    )?;
    Ok(())
}

fn write_spectrum(
    w: &mut impl Write,
    index: usize,
    scan_number: usize,
    rt_min: f32,
    spec: &Spectrum,
) -> io::Result<()> {
    let n = spec.mz.len();

    // Encode m/z as 64-bit floats, intensity as 32-bit floats (Waters native precision).
    let mz_bytes: Vec<u8> = spec.mz.iter().flat_map(|&v| v.to_le_bytes()).collect();
    let int_bytes: Vec<u8> = spec.intensity.iter().flat_map(|&v| v.to_le_bytes()).collect();
    let mz_b64 = base64_encode(&mz_bytes);
    let int_b64 = base64_encode(&int_bytes);

    writeln!(
        w,
        r#"      <spectrum index="{index}" id="scan={scan_number}" defaultArrayLength="{n}">
        <cvParam cvRef="MS" accession="MS:1000511" name="ms level" value="1"/>
        <cvParam cvRef="MS" accession="MS:1000128" name="profile spectrum" value=""/>
        <scanList count="1">
          <cvParam cvRef="MS" accession="MS:1000795" name="no combination" value=""/>
          <scan>
            <cvParam cvRef="MS" accession="MS:1000016" name="scan start time"
              value="{rt_min}" unitCvRef="UO" unitAccession="UO:0000031" unitName="minute"/>
          </scan>
        </scanList>
        <binaryDataArrayList count="2">
          <binaryDataArray>
            <cvParam cvRef="MS" accession="MS:1000514" name="m/z array" value=""/>
            <cvParam cvRef="MS" accession="MS:1000523" name="64-bit float" value=""/>
            <cvParam cvRef="MS" accession="MS:1000576" name="no compression" value=""/>
            <binary>{mz_b64}</binary>
          </binaryDataArray>
          <binaryDataArray>
            <cvParam cvRef="MS" accession="MS:1000515" name="intensity array" value=""/>
            <cvParam cvRef="MS" accession="MS:1000521" name="32-bit float" value=""/>
            <cvParam cvRef="MS" accession="MS:1000576" name="no compression" value=""/>
            <binary>{int_b64}</binary>
          </binaryDataArray>
        </binaryDataArrayList>
      </spectrum>"#
    )?;
    Ok(())
}

fn write_mzml_footer(w: &mut impl Write) -> io::Result<()> {
    writeln!(
        w,
        r#"    </spectrumList>
  </run>
</mzML>"#
    )?;
    Ok(())
}

// ── Utilities ─────────────────────────────────────────────────────────────────

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Minimal base64 encoder (no external dependencies).
fn base64_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let v = (b0 << 16) | (b1 << 8) | b2;
        out.push(ALPHABET[(v >> 18) as usize]);
        out.push(ALPHABET[((v >> 12) & 0x3F) as usize]);
        out.push(if chunk.len() > 1 {
            ALPHABET[((v >> 6) & 0x3F) as usize]
        } else {
            b'='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[(v & 0x3F) as usize]
        } else {
            b'='
        });
    }
    String::from_utf8(out).unwrap()
}

