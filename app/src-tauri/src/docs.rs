//! The document add-on: an attached PDF, Word, Excel, PowerPoint, e-mail or web page is turned into Markdown before
//! the AI reads it, the way markitdown is meant to be used. A PDF read as pages costs an AI the text *and* a picture of
//! every page; Word/Excel/PowerPoint most AIs cannot open at all. The Markdown is a fraction of that.
//!
//! Next to the file the app writes `<name>.digest/DOC.md`; ATTACHMENTS.md tells the AI to read it first and to open
//! the original only for what text cannot carry (a chart, a photo).
//!
//! Which tool converts what comes from the recipes (`tool_recipes.json`, `doc_converters`), never from code:
//! today MarkItDown (Microsoft, MIT) through `uvx`, and OpenThai 2.1 OCR Text (Apache-2.0) for scanned PDFs, whose
//! pages carry no text. Both run through uv (installed by the app when missing). Off with one switch in settings.
use crate::video::{find_file, recipes, tools, Download};
use anyhow::{bail, Context, Result};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::LazyLock;

const OCR_RUNNER: &str = include_str!("ocr_run.py");
/// Markdown longer than this is cut (a huge spreadsheet would cost more than it helps).
const MAX_CHARS: usize = 400_000;

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct DocFile {
    pub path: String,
    pub url: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct DocConverter {
    pub id: String,
    pub label: String,
    /// "uvx": a published tool uv fetches itself · "uv-script": files from `files` + the app's runner, run by uv
    pub kind: String,
    pub license: String,
    pub home: String,
    pub note: String,
    /// file extensions, or "scan" for a PDF whose pages carry no text
    pub formats: Vec<String>,
    pub size_mb: u64,
    pub files: Vec<DocFile>,
    /// {uv} {uvx} {dir} {file}
    pub command: Vec<String>,
}

fn ext(name: &str) -> String {
    name.rsplit_once('.').map(|(_, e)| e.to_lowercase()).unwrap_or_default()
}

fn off_flag(root: &Path) -> PathBuf {
    tools(root).join("docs-off")
}

pub fn enabled(root: &Path) -> bool {
    !off_flag(root).exists()
}

pub fn set_enabled(root: &Path, on: bool) -> Result<()> {
    if on {
        let _ = std::fs::remove_file(off_flag(root));
    } else {
        std::fs::create_dir_all(tools(root))?;
        std::fs::write(off_flag(root), "documents are given to the AI as they are\n")?;
    }
    Ok(())
}

fn exe(name: &str) -> String {
    if cfg!(windows) { format!("{name}.exe") } else { name.to_string() }
}

/// uv and uvx: the add-on's own copy first, else the ones on PATH. `bool` = the add-on's own.
pub fn uv(root: &Path) -> Option<(PathBuf, PathBuf, bool)> {
    if let Some(u) = find_file(&tools(root).join("uv"), &exe("uv"), 2) {
        return Some((u.clone(), u.with_file_name(exe("uvx")), true));
    }
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).find_map(|d| {
        let (u, x) = (d.join(exe("uv")), d.join(exe("uvx")));
        (u.is_file() && x.is_file()).then_some((u, x, false))
    })
}

pub fn conv_dir(root: &Path, id: &str) -> PathBuf {
    tools(root).join("docs").join(id)
}

pub fn ready(root: &Path, c: &DocConverter) -> bool {
    uv(root).is_some()
        && match c.kind.as_str() {
            "uvx" => true,
            "uv-script" => conv_dir(root, &c.id).join("ocr_run.py").is_file() && c.files.iter().all(|f| conv_dir(root, &c.id).join(&f.path).is_file()),
            _ => false,
        }
}

/// Can some converter in the recipes read this kind of file (whether or not it is installed)?
pub fn is_doc(root: &Path, name: &str) -> bool {
    let e = ext(name);
    recipes(root).doc_converters.iter().any(|c| c.formats.iter().any(|f| *f == e))
}

fn pick<'a>(root: &Path, cs: &'a [DocConverter], format: &str) -> Option<&'a DocConverter> {
    cs.iter().find(|c| c.formats.iter().any(|f| f == format) && ready(root, c))
}

pub fn doc_md(file: &Path) -> PathBuf {
    crate::video::digest_dir(file).join("DOC.md")
}

pub fn has_digest(file: &Path) -> bool {
    let (Ok(a), Ok(b)) = (std::fs::metadata(file).and_then(|m| m.modified()), std::fs::metadata(doc_md(file)).and_then(|m| m.modified())) else { return false };
    b >= a
}

/// Will this file get a DOC.md on this machine (on, readable, a converter installed)?
pub fn will_digest(root: &Path, name: &str) -> bool {
    enabled(root) && pick(root, &recipes(root).doc_converters, &ext(name)).is_some()
}

static RUNNING: LazyLock<Mutex<HashSet<PathBuf>>> = LazyLock::new(|| Mutex::new(HashSet::new()));
static ONE_AT_A_TIME: Mutex<()> = parking_lot::const_mutex(());

/// Convert in the background when the file has no Markdown yet (or changed). Safe to call often.
pub fn ensure(file: &Path) {
    let Some(root) = crate::video::root().cloned() else { return };
    let name = file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    if !file.is_file() || has_digest(file) || !will_digest(&root, &name) {
        return;
    }
    let file = file.to_path_buf();
    if !RUNNING.lock().insert(file.clone()) {
        return;
    }
    std::thread::spawn(move || {
        // a file sent in chat moves to attachments/sent/ right after it lands
        std::thread::sleep(std::time::Duration::from_millis(1500));
        let _one = ONE_AT_A_TIME.lock();
        if file.is_file() {
            if let Err(e) = convert(&root, &file) {
                let _ = std::fs::create_dir_all(crate::video::digest_dir(&file));
                let _ = std::fs::write(doc_md(&file), format!("# {name}\n\nแอปแปลงไฟล์นี้เป็นข้อความไม่สำเร็จ: {e:#}\n\nเปิดไฟล์จริงแทน (ถ้า AI ของคุณเปิดได้)\n"));
            }
        }
        RUNNING.lock().remove(&file);
    });
}

fn run_converter(root: &Path, c: &DocConverter, file: &Path) -> Result<String> {
    let (u, ux, _) = uv(root).context("ไม่พบ uv")?;
    let dir = conv_dir(root, &c.id);
    let fill = |a: &str| -> String {
        a.replace("{uv}", &u.to_string_lossy()).replace("{uvx}", &ux.to_string_lossy()).replace("{dir}", &dir.to_string_lossy()).replace("{file}", &file.to_string_lossy())
    };
    let args: Vec<String> = c.command.iter().map(|a| fill(a)).collect();
    let (prog, rest) = args.split_first().context("คำสั่งของตัวแปลงว่าง")?;
    let mut cmd = Command::new(prog);
    cmd.args(rest).env("PYTHONIOENCODING", "utf-8").env("PYTHONUTF8", "1");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let out = cmd.output().with_context(|| format!("เรียก {} ไม่ได้", c.label))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        bail!("{} แปลงไม่สำเร็จ: {}", c.label, err.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").trim());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

pub fn convert(root: &Path, file: &Path) -> Result<()> {
    let name = file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let r = recipes(root);
    let e = ext(&name);
    let c = pick(root, &r.doc_converters, &e).context("ไม่มีตัวแปลงที่ติดตั้งไว้สำหรับไฟล์ชนิดนี้")?;
    let mut text = run_converter(root, c, file)?;
    let mut by = c.label.clone();
    let mut note = String::new();
    // a scanned PDF: its pages are pictures, the text tool finds (almost) nothing
    let size = std::fs::metadata(file).map(|m| m.len()).unwrap_or(0);
    if e == "pdf" && text.trim().chars().count() < 100 && size > 20_000 {
        match pick(root, &r.doc_converters, "scan") {
            Some(ocr) => {
                text = run_converter(root, ocr, file)?;
                by = ocr.label.clone();
            }
            None => note = "\n> ไฟล์นี้เป็นภาพสแกน (ไม่มีตัวหนังสือในไฟล์) และเครื่องนี้ยังไม่ได้ติดตั้ง OCR — เปิดไฟล์จริงเพื่อดูภาพ หรือขอเจ้าของติดตั้ง OCR ในส่วนเสริม\n".into(),
        }
    }
    let chars = text.chars().count();
    if chars > MAX_CHARS {
        text = text.chars().take(MAX_CHARS).collect::<String>();
        note.push_str(&format!("\n> ตัดเหลือ {MAX_CHARS} ตัวอักษรแรกจาก {chars} — ถ้าต้องการส่วนที่เหลือ เปิดไฟล์จริงเฉพาะส่วนนั้น\n"));
    }
    let head = format!(
        "# เอกสาร: {name}\n\nแปลงเป็นข้อความโดยแอป ({by}) เพื่อประหยัด token — **อ่านไฟล์นี้แทนการเปิดไฟล์ต้นฉบับ** เปิดต้นฉบับเฉพาะเมื่อต้องดูภาพ กราฟ หรือหน้าตาเอกสาร\n- ต้นฉบับ {} KB → ข้อความ {} KB{note}\n\n---\n\n",
        size.div_ceil(1024),
        text.len().div_ceil(1024)
    );
    std::fs::create_dir_all(crate::video::digest_dir(file))?;
    std::fs::write(doc_md(file), format!("{head}{text}"))?;
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------

pub fn status(root: &Path) -> Value {
    let r = recipes(root);
    let u = uv(root);
    let convs: Vec<Value> = r
        .doc_converters
        .iter()
        .map(|c| json!({ "id": c.id, "label": c.label, "note": c.note, "license": c.license, "home": c.home, "kind": c.kind, "formats": c.formats, "size_mb": c.size_mb, "ready": ready(root, c) }))
        .collect();
    json!({
        "enabled": enabled(root),
        "uv": { "found": u.is_some(), "own": u.as_ref().is_some_and(|x| x.2), "size_mb": r.uv.size_mb, "label": r.uv.label },
        "converters": convs,
    })
}

/// "uv" or "conv:<id>" (uv comes along when missing). Called from video::install, which owns the progress job.
pub async fn install(root: &Path, part: &str, uv_dl: &Download) -> Result<()> {
    if uv(root).is_none() {
        crate::video::job_message(format!("กำลังดาวน์โหลด {}", uv_dl.label));
        crate::video::fetch_unpacked(&uv_dl.url, &uv_dl.sha256, uv_dl.size_mb, &tools(root).join("uv")).await?;
    }
    if part == "uv" {
        return Ok(());
    }
    let id = part.strip_prefix("conv:").context("ไม่รู้จักส่วนที่จะติดตั้ง")?;
    let r = recipes(root);
    let c = r.doc_converters.iter().find(|c| c.id == id).context("ไม่รู้จักตัวแปลงนี้")?.clone();
    if c.kind == "uv-script" {
        let dir = conv_dir(root, &c.id);
        for f in &c.files {
            crate::video::job_message(format!("กำลังดาวน์โหลด {} · {}", c.label, f.path));
            crate::video::download(&f.url, &f.sha256, &dir.join(&f.path), c.size_mb as f64).await?;
        }
        std::fs::write(dir.join("ocr_run.py"), OCR_RUNNER)?;
    }
    // fetch the tool's own parts now (uv caches them), so the first document is not the slow one
    crate::video::job_message(format!("กำลังเตรียม {} (ครั้งแรกเท่านั้น)", c.label));
    let probe = std::env::temp_dir().join("ia-docs-probe.html");
    std::fs::write(&probe, "<html><body><p>ok</p></body></html>")?;
    let root2 = root.to_path_buf();
    let c2 = c.clone();
    let p2 = probe.clone();
    let warm = tauri::async_runtime::spawn_blocking(move || if c2.kind == "uvx" { run_converter(&root2, &c2, &p2).map(|_| ()) } else { Ok(()) }).await;
    let _ = std::fs::remove_file(&probe);
    warm.map_err(|e| anyhow::anyhow!("{e}"))??;
    Ok(())
}

pub fn remove(root: &Path, part: &str) -> Result<()> {
    if part == "uv" {
        let _ = std::fs::remove_dir_all(tools(root).join("uv"));
        return Ok(());
    }
    let id = part.strip_prefix("conv:").context("ไม่รู้จักส่วนที่จะลบ")?;
    let _ = std::fs::remove_dir_all(conv_dir(root, id));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recipes_have_a_converter_for_office_files_and_scans() {
        let r: crate::video::Recipes = serde_json::from_str(include_str!("tool_recipes.json")).unwrap();
        for f in ["pdf", "docx", "xlsx", "pptx", "scan"] {
            assert!(r.doc_converters.iter().any(|c| c.formats.iter().any(|x| x == f)), "{f}");
        }
        for c in &r.doc_converters {
            assert!(c.command.iter().any(|a| a.contains("{file}")), "{}", c.id);
            assert!(c.files.iter().all(|f| f.sha256.len() == 64), "{}", c.id);
        }
        assert_eq!(r.uv.sha256.len(), 64);
    }

    #[test]
    fn switch_is_a_file() {
        let root = std::env::temp_dir().join(format!("ia-docs-{}", std::process::id()));
        assert!(enabled(&root));
        set_enabled(&root, false).unwrap();
        assert!(!enabled(&root));
        set_enabled(&root, true).unwrap();
        assert!(enabled(&root));
        let _ = std::fs::remove_dir_all(&root);
    }

    /// MarkItDown through uv on a real file: DOC_TEST_FILE=<file> cargo test --lib -- --ignored docs_real
    #[test]
    #[ignore]
    fn docs_real_file() {
        let f = PathBuf::from(std::env::var("DOC_TEST_FILE").unwrap());
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
        convert(&root, &f).unwrap();
        let md = std::fs::read_to_string(doc_md(&f)).unwrap();
        eprintln!("{}", md.chars().take(600).collect::<String>());
        assert!(md.len() > 500);
    }
}
