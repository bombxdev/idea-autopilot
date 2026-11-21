//! `ia speech …` — run a speech-to-text engine that lives outside the app (the video & sound add-on, video.rs).
//!
//! The engine is loaded only here, in a short-lived `ia.exe` process: the app itself never links it, so a machine
//! without the add-on carries nothing, and an engine that crashes takes down this process, not the app.
//!
//! Engine kind "transcribe-dll": transcribe.cpp's C API (MIT, many model families from one GGUF file). Every
//! parameter struct is passed as NULL (library defaults), so only stable entry points are used:
//! init_backends → open → run → full_text / detected_language → close.
//!
//! Input: 16 kHz mono f32le PCM (written by ffmpeg) and the pieces to transcribe, cut at pauses by the app
//! (`--cuts 0.0-12.4,13.1-30.0`, seconds). Output: one JSON line per piece: {"i","t0","t1","text","lang"}.
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::io::Write;
use std::path::{Path, PathBuf};

type InitBackends = unsafe extern "C" fn(*const c_char) -> c_int;
type InitDefault = unsafe extern "C" fn() -> c_int;
type Open = unsafe extern "C" fn(*const c_char, *const c_void, *const c_void, *mut *mut c_void) -> c_int;
type Run = unsafe extern "C" fn(*mut c_void, *const f32, c_int, *const c_void) -> c_int;
type Text = unsafe extern "C" fn(*const c_void) -> *const c_char;
type Close = unsafe extern "C" fn(*mut c_void);
type StatusString = unsafe extern "C" fn(c_int) -> *const c_char;
type RunParamsInit = unsafe extern "C" fn(*mut c_void);

/// The leading fields of `struct transcribe_run_params` (transcribe.h). The library only ever appends fields, so this
/// prefix stays valid; the struct itself lives in a larger zeroed buffer that `transcribe_run_params_init` fills.
#[repr(C)]
struct RunParamsHead {
    struct_size: u64,
    task: c_int,
    timestamps: c_int,
    pnc: c_int,
    itn: c_int,
    diarize: c_int,
    language: *const c_char,
}

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}

/// "0.0-12.4,13.1-30" → [(0.0, 12.4), (13.1, 30.0)]
pub fn parse_cuts(s: &str) -> Vec<(f64, f64)> {
    s.split(',')
        .filter_map(|p| {
            let (a, b) = p.trim().split_once('-')?;
            let (a, b) = (a.trim().parse::<f64>().ok()?, b.trim().parse::<f64>().ok()?);
            (b > a && a >= 0.0).then_some((a, b))
        })
        .collect()
}

fn read_pcm(p: &Path) -> Result<Vec<f32>, String> {
    let bytes = std::fs::read(p).map_err(|e| format!("อ่านเสียงไม่ได้: {e}"))?;
    Ok(bytes.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).map(|x| if x.is_finite() { x.clamp(-1.0, 1.0) } else { 0.0 }).collect())
}

unsafe fn cstr(p: *const c_char) -> String {
    if p.is_null() { String::new() } else { CStr::from_ptr(p).to_string_lossy().into_owned() }
}

/// The folder an engine was unpacked into keeps its DLL one level down (`transcribe-native-…/transcribe.dll`).
pub fn find_dll(dir: &Path, name: &str) -> Option<PathBuf> {
    if dir.join(name).is_file() {
        return Some(dir.join(name));
    }
    std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).filter(|p| p.is_dir()).find_map(|d| d.join(name).is_file().then(|| d.join(name)))
}

#[cfg(windows)]
unsafe fn load(dll: &Path) -> Result<libloading::Library, String> {
    // its ggml-*.dll neighbours must be found next to it, not on PATH
    const LOAD_WITH_ALTERED_SEARCH_PATH: u32 = 0x8;
    libloading::os::windows::Library::load_with_flags(dll, LOAD_WITH_ALTERED_SEARCH_PATH).map(Into::into).map_err(|e| format!("โหลด {} ไม่ได้: {e}", dll.display()))
}

#[cfg(not(windows))]
unsafe fn load(dll: &Path) -> Result<libloading::Library, String> {
    libloading::Library::new(dll).map_err(|e| format!("โหลด {} ไม่ได้: {e}", dll.display()))
}

pub fn cli(args: &[String]) -> Result<(), String> {
    let kind = flag(args, "--kind").unwrap_or_else(|| "transcribe-dll".into());
    if kind != "transcribe-dll" {
        return Err(format!("ไม่รู้จักชนิดเครื่องมือถอดเสียง “{kind}”"));
    }
    let dir = PathBuf::from(flag(args, "--engine-dir").ok_or("ต้องมี --engine-dir")?);
    let model = flag(args, "--model").ok_or("ต้องมี --model")?;
    let pcm = read_pcm(Path::new(&flag(args, "--pcm").ok_or("ต้องมี --pcm")?))?;
    let cuts = match flag(args, "--cuts") {
        Some(c) => parse_cuts(&c),
        None => vec![(0.0, pcm.len() as f64 / 16000.0)],
    };
    let limit: usize = flag(args, "--limit").and_then(|x| x.parse().ok()).unwrap_or(usize::MAX);
    let dll = find_dll(&dir, if cfg!(windows) { "transcribe.dll" } else { "libtranscribe.so" }).ok_or("ไม่พบไฟล์ของเครื่องมือถอดเสียง (ติดตั้งส่วนเสริมใหม่)")?;
    unsafe {
        let lib = load(&dll)?;
        let sym = |n: &[u8]| -> Result<*const c_void, String> {
            lib.get::<*const c_void>(n).map(|s| *s).map_err(|e| format!("เครื่องมือถอดเสียงรุ่นนี้ใช้ไม่ได้ ({e})"))
        };
        let init: InitBackends = std::mem::transmute(sym(b"transcribe_init_backends\0")?);
        let init_default: InitDefault = std::mem::transmute(sym(b"transcribe_init_backends_default\0")?);
        let open: Open = std::mem::transmute(sym(b"transcribe_open\0")?);
        let run: Run = std::mem::transmute(sym(b"transcribe_run\0")?);
        let full_text: Text = std::mem::transmute(sym(b"transcribe_full_text\0")?);
        let detected: Text = std::mem::transmute(sym(b"transcribe_detected_language\0")?);
        let close: Close = std::mem::transmute(sym(b"transcribe_close\0")?);
        let status: StatusString = std::mem::transmute(sym(b"transcribe_status_string\0")?);
        let why = |s: c_int| format!("{} ({s})", cstr(status(s)));
        let params_init: RunParamsInit = std::mem::transmute(sym(b"transcribe_run_params_init\0")?);
        // the language to listen for ("th", "en"…); none = the model decides (Whisper detects it)
        let lang = flag(args, "--lang").filter(|l| !l.is_empty() && l != "auto").and_then(|l| CString::new(l).ok());
        let mut params_buf = vec![0u64; 128];
        let params: *const c_void = match &lang {
            Some(l) => {
                params_init(params_buf.as_mut_ptr() as *mut c_void);
                (*(params_buf.as_mut_ptr() as *mut RunParamsHead)).language = l.as_ptr();
                params_buf.as_ptr() as *const c_void
            }
            None => std::ptr::null(),
        };

        let backends = CString::new(dll.parent().unwrap_or(&dir).to_string_lossy().as_bytes()).map_err(|e| e.to_string())?;
        if init(backends.as_ptr()) != 0 && init_default() != 0 {
            return Err("เริ่มเครื่องมือถอดเสียงไม่ได้ (ไม่พบตัวประมวลผล)".into());
        }
        let model_c = CString::new(model.as_bytes()).map_err(|e| e.to_string())?;
        let mut sess: *mut c_void = std::ptr::null_mut();
        let s = open(model_c.as_ptr(), std::ptr::null(), std::ptr::null(), &mut sess);
        if s != 0 || sess.is_null() {
            return Err(format!("เปิดโมเดลไม่ได้: {}", why(s)));
        }
        let mut out = std::io::stdout().lock();
        for (i, (t0, t1)) in cuts.iter().enumerate().take(limit) {
            let a = ((t0 * 16000.0) as usize).min(pcm.len());
            let b = ((t1 * 16000.0) as usize).min(pcm.len());
            if b <= a + 1600 {
                continue; // under 0.1 s: nothing to hear
            }
            let s = run(sess, pcm[a..b].as_ptr(), (b - a) as c_int, params);
            let (text, lang) = if s == 0 { (cstr(full_text(sess)), cstr(detected(sess))) } else { (String::new(), String::new()) };
            let line = serde_json::json!({ "i": i, "t0": t0, "t1": t1, "text": text.trim(), "lang": lang, "error": if s == 0 { String::new() } else { why(s) } });
            let _ = writeln!(out, "{line}");
            let _ = out.flush();
        }
        close(sess);
        drop(lib);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cuts_are_read_and_bad_ones_dropped() {
        assert_eq!(parse_cuts("0-1.5, 2.25-4,5-3,x-1"), vec![(0.0, 1.5), (2.25, 4.0)]);
    }
}
