//! The video & sound add-on: an attached video (or sound file) becomes something every AI can read for few tokens,
//! the way markitdown does for documents.
//!
//! Next to the file the app writes `<name>.digest/`:
//! - `VIDEO.md` — length, shape, the shot list with times, cutting pace, colours, loudness and (when a speech model
//!   is installed) what is said, with times. Plain text: a few hundred tokens instead of a whole video.
//! - `sheet.jpg` — up to 16 frames on one picture (about 1,500 tokens to look at), and `frames/fNN.jpg` one per shot,
//!   opened only when a shot matters.
//!
//! Nothing here is tied to one tool. What runs comes from recipes (`tool_recipes.json`, overridden by
//! `data/tools/recipes.json`): ffmpeg for pictures and sound, and a speech engine + model chosen per language by
//! score (OpenThai 2.0 ASR for Thai, Whisper for the rest today). A better engine or model is a new recipe entry, not
//! new code. All of it is optional and downloaded only when the owner turns it on: without it the app carries none of
//! it, and a video still gets the picture part when ffmpeg is on the machine.
use anyhow::{bail, Context, Result};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{LazyLock, OnceLock};

const DEFAULT_RECIPES: &str = include_str!("tool_recipes.json");
/// Frames on the contact sheet (4 across).
const SHEET_MAX: usize = 16;
/// Speech longer than this is not transcribed past it (a long film would hold the machine for too long).
const SPEECH_MAX_SECS: f64 = 60.0 * 60.0;

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Download {
    pub label: String,
    pub license: String,
    pub home: String,
    pub url: String,
    pub sha256: String,
    pub size_mb: u64,
    pub unpack: bool,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Engine {
    pub id: String,
    pub label: String,
    /// how `ia speech` drives it: "transcribe-dll" today
    pub kind: String,
    pub license: String,
    pub home: String,
    pub url: String,
    pub sha256: String,
    pub size_mb: u64,
    pub unpack: bool,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct SpeechModel {
    pub id: String,
    pub label: String,
    pub engine: String,
    /// language code → how good it is there (0–100); "*" = any other language
    pub langs: BTreeMap<String, u32>,
    pub detects_language: bool,
    pub license: String,
    pub home: String,
    pub note: String,
    pub url: String,
    pub file: String,
    pub sha256: String,
    pub size_mb: u64,
}

impl SpeechModel {
    pub fn score(&self, lang: &str) -> u32 {
        self.langs.get(lang).or_else(|| self.langs.get("*")).copied().unwrap_or(0)
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Recipes {
    pub version: u32,
    pub ffmpeg: Download,
    pub engines: Vec<Engine>,
    pub speech_models: Vec<SpeechModel>,
    /// uv: runs the Python tools of the document add-on (docs.rs)
    pub uv: Download,
    pub doc_converters: Vec<crate::docs::DocConverter>,
}

static ROOT: OnceLock<PathBuf> = OnceLock::new();

/// Called once at start-up, so attachments written from anywhere can be digested.
pub fn set_root(root: PathBuf) {
    let _ = ROOT.set(root);
}

pub fn root() -> Option<&'static PathBuf> {
    ROOT.get()
}

pub fn tools(root: &Path) -> PathBuf {
    root.join("data").join("tools")
}

/// The machine's own recipes (data/tools/recipes.json) when they read, else the ones built in.
pub fn recipes(root: &Path) -> Recipes {
    std::fs::read_to_string(tools(root).join("recipes.json"))
        .ok()
        .and_then(|s| serde_json::from_str::<Recipes>(&s).ok())
        .filter(|r| r.version > 0)
        .unwrap_or_else(|| serde_json::from_str(DEFAULT_RECIPES).expect("tool_recipes.json"))
}

fn exe(name: &str) -> String {
    if cfg!(windows) { format!("{name}.exe") } else { name.to_string() }
}

pub(crate) fn find_file(dir: &Path, name: &str, depth: usize) -> Option<PathBuf> {
    if dir.join(name).is_file() {
        return Some(dir.join(name));
    }
    if depth == 0 {
        return None;
    }
    std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).filter(|p| p.is_dir()).find_map(|d| find_file(&d, name, depth - 1))
}

/// ffmpeg and ffprobe: the add-on's own copy first, else the one on PATH. `bool` = the add-on's own.
pub fn ffmpeg(root: &Path) -> Option<(PathBuf, PathBuf, bool)> {
    if let Some(f) = find_file(&tools(root).join("ffmpeg"), &exe("ffmpeg"), 3) {
        let p = f.with_file_name(exe("ffprobe"));
        if p.is_file() {
            return Some((f, p, true));
        }
    }
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).find_map(|d| {
        let (f, p) = (d.join(exe("ffmpeg")), d.join(exe("ffprobe")));
        (f.is_file() && p.is_file()).then_some((f, p, false))
    })
}

pub fn engine_dir(root: &Path, id: &str) -> PathBuf {
    tools(root).join("speech").join(id)
}

pub fn engine_ready(root: &Path, e: &Engine) -> bool {
    match e.kind.as_str() {
        "transcribe-dll" => find_file(&engine_dir(root, &e.id), if cfg!(windows) { "transcribe.dll" } else { "libtranscribe.so" }, 2).is_some(),
        _ => false,
    }
}

pub fn model_path(root: &Path, m: &SpeechModel) -> PathBuf {
    tools(root).join("speech").join("models").join(&m.file)
}

/// Installed speech models whose engine is installed too, best for `lang` first.
pub fn usable_models(root: &Path, r: &Recipes, lang: &str) -> Vec<(SpeechModel, Engine)> {
    let mut v: Vec<(SpeechModel, Engine)> = r
        .speech_models
        .iter()
        .filter(|m| !m.file.is_empty() && model_path(root, m).is_file())
        .filter_map(|m| r.engines.iter().find(|e| e.id == m.engine && engine_ready(root, e)).map(|e| (m.clone(), e.clone())))
        .collect();
    v.sort_by(|a, b| b.0.score(lang).cmp(&a.0.score(lang)));
    v
}

/// The model to suggest for an owner who works in `lang`: the best scorer there.
pub fn recommended(r: &Recipes, lang: &str) -> Option<SpeechModel> {
    r.speech_models.iter().max_by_key(|m| m.score(lang)).cloned()
}

// ---------------------------------------------------------------------------------------------------------------
// Digest
// ---------------------------------------------------------------------------------------------------------------

pub fn digest_dir(media: &Path) -> PathBuf {
    let name = media.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    media.with_file_name(format!("{name}.digest"))
}

pub fn is_media(name: &str) -> bool {
    matches!(crate::attach::kind_of(name), "video" | "audio")
}

/// Has this file a digest that is at least as new as the file?
pub fn has_digest(media: &Path) -> bool {
    let md = digest_dir(media).join("VIDEO.md");
    let (Ok(a), Ok(b)) = (std::fs::metadata(media).and_then(|m| m.modified()), std::fs::metadata(&md).and_then(|m| m.modified())) else { return false };
    b >= a
}

static RUNNING: LazyLock<Mutex<HashSet<PathBuf>>> = LazyLock::new(|| Mutex::new(HashSet::new()));
/// One digest at a time: speech models are big and the owner's machine is busy enough.
static ONE_AT_A_TIME: Mutex<()> = parking_lot::const_mutex(());

/// Digest in the background when the file has none yet (or changed). Safe to call often.
pub fn ensure(media: &Path) {
    let Some(root) = root().cloned() else { return };
    let name = media.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    if !is_media(&name) || !media.is_file() || has_digest(media) {
        return;
    }
    let media = media.to_path_buf();
    if !RUNNING.lock().insert(media.clone()) {
        return;
    }
    std::thread::spawn(move || {
        // a file sent in chat is moved to attachments/sent/ right after it lands: digest it where it stays
        std::thread::sleep(std::time::Duration::from_millis(1500));
        let _one = ONE_AT_A_TIME.lock();
        if !media.is_file() {
            RUNNING.lock().remove(&media);
            return;
        }
        if let Err(e) = digest(&root, &media, &crate::lang::ui()) {
            let _ = std::fs::create_dir_all(digest_dir(&media));
            let _ = std::fs::write(digest_dir(&media).join("VIDEO.md"), format!("# {name}\n\nแอปสรุปไฟล์นี้ไม่สำเร็จ: {e:#}\n\nเปิดไฟล์เองถ้า AI ของคุณเปิดได้ หรือบอกเจ้าของใน ISSUES.md\n"));
        }
        RUNNING.lock().remove(&media);
    });
}

/// Every video/sound file in an idea's attachments (and the ones sent in chat) gets a digest.
pub fn ensure_idea(idea_dir: &Path) {
    let base = idea_dir.join("attachments");
    for d in [base.clone(), base.join("sent")] {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_file() {
                ensure(&p);
                crate::docs::ensure(&p);
            }
        }
    }
}

fn run(cmd: &mut Command) -> Result<std::process::Output> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // no console window
    }
    cmd.output().context("เรียก ffmpeg ไม่ได้")
}

#[derive(Debug, Default, Clone)]
struct Probe {
    duration: f64,
    width: u32,
    height: u32,
    fps: f64,
    video: bool,
    audio: bool,
}

fn probe(ffprobe: &Path, media: &Path) -> Result<Probe> {
    let out = run(Command::new(ffprobe).args(["-v", "error", "-print_format", "json", "-show_format", "-show_streams"]).arg(media))?;
    let v: Value = serde_json::from_slice(&out.stdout).context("อ่านข้อมูลไฟล์ไม่ได้ (ไฟล์อาจเสีย)")?;
    let mut p = Probe { duration: v["format"]["duration"].as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0), ..Default::default() };
    for s in v["streams"].as_array().cloned().unwrap_or_default() {
        match s["codec_type"].as_str() {
            // a cover picture inside a sound file is not a video
            Some("video") if s["disposition"]["attached_pic"].as_i64() != Some(1) => {
                p.video = true;
                p.width = s["width"].as_u64().unwrap_or(0) as u32;
                p.height = s["height"].as_u64().unwrap_or(0) as u32;
                p.fps = s["avg_frame_rate"].as_str().and_then(|r| r.split_once('/')).and_then(|(a, b)| Some(a.parse::<f64>().ok()? / b.parse::<f64>().ok().filter(|x| *x > 0.0)?)).unwrap_or(0.0);
            }
            Some("audio") => p.audio = true,
            _ => {}
        }
    }
    if p.duration <= 0.0 {
        bail!("ไฟล์นี้ไม่มีความยาว (อาจไม่ใช่วิดีโอหรือเสียง)");
    }
    Ok(p)
}

/// Times (seconds) where the picture changes shot. Every frame's change score is read, and a cut is a clear peak
/// above this clip's own level (fast motion scores high on many frames in a row; a soft cut in a letterboxed clip
/// scores low): above max(0.10, mean + 4·sd) and the highest within 0.6 s.
fn scene_cuts(ffmpeg: &Path, media: &Path) -> Vec<f64> {
    let Ok(out) = run(Command::new(ffmpeg).args(["-hide_banner", "-nostats", "-i"]).arg(media).args(["-an", "-sn", "-vf", "scale=160:-2,select='gte(scene,0)',metadata=print:key=lavfi.scene_score", "-f", "null", "-"])) else { return vec![] };
    let mut scores: Vec<(f64, f64)> = vec![];
    let mut t = None;
    for l in String::from_utf8_lossy(&out.stderr).lines() {
        if let Some(x) = l.split("pts_time:").nth(1) {
            t = x.split_whitespace().next().and_then(|v| v.parse::<f64>().ok());
        } else if let (Some(x), Some(tt)) = (l.split("lavfi.scene_score=").nth(1), t) {
            if let Ok(v) = x.trim().parse::<f64>() {
                scores.push((tt, v));
            }
        }
    }
    cuts_from_scores(&scores)
}

pub fn cuts_from_scores(scores: &[(f64, f64)]) -> Vec<f64> {
    if scores.is_empty() {
        return vec![];
    }
    let n = scores.len() as f64;
    let mean = scores.iter().map(|s| s.1).sum::<f64>() / n;
    let sd = (scores.iter().map(|s| (s.1 - mean).powi(2)).sum::<f64>() / n).sqrt();
    let thr = (mean + 4.0 * sd).max(0.10);
    let mut cuts: Vec<(f64, f64)> = vec![];
    for &(t, v) in scores.iter().filter(|s| s.1 > thr) {
        match cuts.last_mut() {
            Some(last) if t - last.0 < 0.6 => {
                if v > last.1 {
                    *last = (t, v);
                }
            }
            _ => cuts.push((t, v)),
        }
    }
    cuts.into_iter().map(|c| c.0).collect()
}

/// Shots from cut times: every shot at least 0.25 s.
pub fn shots(cuts: &[f64], duration: f64) -> Vec<(f64, f64)> {
    let mut edges = vec![0.0];
    for &c in cuts {
        if c > *edges.last().unwrap() + 0.25 && c < duration - 0.25 {
            edges.push(c);
        }
    }
    edges.push(duration);
    edges.windows(2).map(|w| (w[0], w[1])).collect()
}

/// Which moments get a picture: the middle of each shot, spread evenly when there are more shots than room; a long
/// single shot (a talking head) still gets a few frames over its length.
pub fn picks(shots: &[(f64, f64)], max: usize) -> Vec<(usize, f64)> {
    // one frame per shot, and one per 10 s inside a long shot (a slow crossfade hides changes from the cut finder)
    let all: Vec<(usize, f64)> = shots
        .iter()
        .enumerate()
        .flat_map(|(i, &(a, b))| {
            let n = (((b - a) / 10.0).ceil() as usize).clamp(1, 8);
            (0..n).map(move |k| (i, a + (b - a) * (k as f64 + 0.5) / n as f64))
        })
        .collect();
    if all.len() <= max {
        return all;
    }
    (0..max).map(|k| all[k * (all.len() - 1) / (max - 1)]).collect()
}

/// 3×3 average colours of a picture.
fn colours(ffmpeg: &Path, img: &Path) -> Vec<[u8; 3]> {
    let Ok(out) = run(Command::new(ffmpeg).args(["-v", "error", "-i"]).arg(img).args(["-vf", "scale=3:3:flags=area", "-f", "rawvideo", "-pix_fmt", "rgb24", "-"])) else { return vec![] };
    out.stdout.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect()
}

fn hex(c: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

fn mean(cs: &[[u8; 3]]) -> [u8; 3] {
    if cs.is_empty() {
        return [0, 0, 0];
    }
    let n = cs.len() as u32;
    let s = cs.iter().fold([0u32; 3], |a, c| [a[0] + c[0] as u32, a[1] + c[1] as u32, a[2] + c[2] as u32]);
    [(s[0] / n) as u8, (s[1] / n) as u8, (s[2] / n) as u8]
}

fn light(c: [u8; 3]) -> &'static str {
    let y = 0.2126 * c[0] as f64 + 0.7152 * c[1] as f64 + 0.0722 * c[2] as f64;
    match y as u32 {
        0..=50 => "มืด",
        51..=110 => "ค่อนข้างมืด",
        111..=180 => "กลาง",
        _ => "สว่าง",
    }
}

/// The main colours over all frames: close colours share a bucket, the five fullest buckets with their share.
pub fn palette(all: &[[u8; 3]]) -> Vec<(String, u32)> {
    let mut b: BTreeMap<(u8, u8, u8), Vec<[u8; 3]>> = BTreeMap::new();
    for c in all {
        b.entry((c[0] >> 6, c[1] >> 6, c[2] >> 6)).or_default().push(*c);
    }
    let mut v: Vec<(usize, [u8; 3])> = b.values().map(|cs| (cs.len(), mean(cs))).collect();
    v.sort_by(|a, b| b.0.cmp(&a.0));
    let total = all.len().max(1) as f64;
    v.into_iter().take(5).map(|(n, c)| (hex(c), (n as f64 * 100.0 / total).round() as u32)).collect()
}

fn mmss(t: f64) -> String {
    let t = t.max(0.0);
    let m = (t / 60.0).floor() as u64;
    let s = t - m as f64 * 60.0;
    format!("{m}:{s:04.1}")
}

fn mmss_short(t: f64) -> String {
    let t = t.max(0.0).round() as u64;
    format!("{}:{:02}", t / 60, t % 60)
}

/// Where to cut the sound for the speech model: at the quietest moment 6–20 s into each piece (music under the voice
/// leaves no true silence, so the quietest point is the pause), skipping pieces with nothing to hear. Also returns
/// the share of the clip that has sound.
pub fn speech_cuts(pcm: &[f32]) -> (Vec<(f64, f64)>, f64) {
    const FRAME: usize = 320; // 20 ms at 16 kHz
    let db: Vec<f32> = pcm
        .chunks(FRAME)
        .map(|c| {
            let e = c.iter().map(|x| x * x).sum::<f32>() / c.len().max(1) as f32;
            10.0 * (e.max(1e-10)).log10()
        })
        .collect();
    if db.is_empty() {
        return (vec![], 0.0);
    }
    let mut sorted = db.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let floor = sorted[sorted.len() / 10];
    let quiet = (floor + 6.0).clamp(-50.0, -30.0);
    let loud_share = db.iter().filter(|d| **d > quiet).count() as f64 / db.len() as f64;
    let fps = 50.0; // frames per second
    let (lo, hi) = ((6.0 * fps) as usize, (20.0 * fps) as usize);
    let mut cuts = vec![];
    let mut start = 0usize;
    while start < db.len() {
        let end = if db.len() - start <= hi {
            db.len()
        } else {
            let w = &db[start + lo..start + hi];
            start + lo + w.iter().enumerate().min_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal)).map(|(i, _)| i).unwrap_or(0)
        };
        let piece = &db[start..end];
        let heard = piece.iter().filter(|d| **d > quiet).count() as f64 / piece.len().max(1) as f64;
        if heard > 0.1 {
            cuts.push((start as f64 / fps, end as f64 / fps));
        }
        start = end.max(start + 1);
    }
    (cuts, loud_share)
}

fn read_f32(p: &Path) -> Vec<f32> {
    std::fs::read(p).map(|b| b.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect()).unwrap_or_default()
}

/// `ia.exe` next to the running app: it runs the speech engine in its own process.
pub(crate) fn ia_exe() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("IA_SPEECH_EXE").map(PathBuf::from).filter(|p| p.is_file()) {
        return Some(p); // tests
    }
    let me = std::env::current_exe().ok()?;
    let p = me.with_file_name(exe("ia"));
    p.is_file().then_some(p)
}

#[derive(Debug, Clone, Deserialize)]
struct Heard {
    t0: f64,
    #[allow(dead_code)]
    t1: f64,
    text: String,
    #[serde(default)]
    lang: String,
}

fn transcribe(root: &Path, eng: &Engine, model: &SpeechModel, pcm: &Path, cuts: &[(f64, f64)], lang: &str, limit: Option<usize>) -> Result<Vec<Heard>> {
    let ia = ia_exe().context("ไม่พบ ia.exe ข้างโปรแกรม")?;
    let cut_arg = cuts.iter().map(|(a, b)| format!("{a:.2}-{b:.2}")).collect::<Vec<_>>().join(",");
    let mut c = Command::new(ia);
    c.args(["speech", "--kind", &eng.kind, "--engine-dir"]).arg(engine_dir(root, &eng.id)).arg("--model").arg(model_path(root, model)).arg("--pcm").arg(pcm).args(["--cuts", &cut_arg]);
    if !lang.is_empty() {
        c.args(["--lang", lang]);
    }
    if let Some(n) = limit {
        c.args(["--limit", &n.to_string()]);
    }
    let out = run(&mut c)?;
    if !out.status.success() {
        bail!("ถอดเสียงไม่สำเร็จ: {}", String::from_utf8_lossy(&out.stderr).lines().last().unwrap_or("").trim());
    }
    Ok(String::from_utf8_lossy(&out.stdout).lines().filter_map(|l| serde_json::from_str::<Heard>(l).ok()).collect())
}

/// Pick the model for this clip: the best one for the owner's language; when it cannot tell languages apart and a
/// model that can is installed, that one listens to the start first and the clip's own language decides.
fn choose(root: &Path, r: &Recipes, pref: &str, pcm: &Path, cuts: &[(f64, f64)]) -> Option<(SpeechModel, Engine, String)> {
    let models = usable_models(root, r, pref);
    let (best, eng) = models.first()?.clone();
    if best.detects_language {
        return Some((best, eng, String::new()));
    }
    if let Some((det, det_eng)) = models.iter().find(|(m, _)| m.detects_language) {
        if let Ok(h) = transcribe(root, det_eng, det, pcm, cuts, "", Some(2)) {
            if let Some(l) = h.iter().map(|x| x.lang.clone()).find(|l| !l.is_empty()) {
                let top = usable_models(root, r, &l).into_iter().next()?;
                let lang = if top.0.detects_language { String::new() } else { l };
                return Some((top.0, top.1, lang));
            }
        }
    }
    let lang = best.langs.keys().find(|k| *k != "*").cloned().unwrap_or_default();
    Some((best, eng, lang))
}

const SPEECH_MARK: &str = "<!-- speech -->";

pub fn digest(root: &Path, media: &Path, pref_lang: &str) -> Result<()> {
    let name = media.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let out = digest_dir(media);
    let Some((ff, fp, _)) = ffmpeg(root) else {
        std::fs::create_dir_all(&out)?;
        std::fs::write(
            out.join("VIDEO.md"),
            format!("# {name}\n\nเครื่องนี้ยังไม่มีส่วนเสริม \"เข้าใจวิดีโอและเสียง\" แอปจึงสรุปไฟล์นี้ให้ไม่ได้ — ถ้าต้องใช้เนื้อหาในไฟล์ ให้ขอเจ้าของเปิดที่ ตั้งค่า → AI และโมเดล → ส่วนเสริม (ใส่ใน `needs_from_user`) แล้วทำส่วนอื่นต่อไป\n"),
        )?;
        return Ok(());
    };
    let p = probe(&fp, media)?;
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(out.join("frames"))?;

    let mut md = format!("# {} : {name}\n\nสรุปจากไฟล์จริงโดยแอป (ffmpeg) เพื่อประหยัด token — **อ่านไฟล์นี้แทนการเปิดไฟล์ต้นฉบับ**\n\n", if p.video { "วิดีโอ" } else { "เสียง" });
    md.push_str(&format!("- ความยาว {}", mmss_short(p.duration)));
    if p.video {
        let shape = match (p.width, p.height) {
            (w, h) if h > w => "แนวตั้ง",
            (w, h) if w > h => "แนวนอน",
            _ => "สี่เหลี่ยมจัตุรัส",
        };
        md.push_str(&format!(" · {}×{} ({shape}) · {:.0} fps", p.width, p.height, p.fps));
    }
    md.push_str(if p.audio { " · มีเสียง\n" } else { " · ไม่มีเสียง\n" });

    if p.video {
        let cuts = scene_cuts(&ff, media);
        let sh = shots(&cuts, p.duration);
        let lens: Vec<f64> = sh.iter().map(|(a, b)| b - a).collect();
        let avg = lens.iter().sum::<f64>() / lens.len().max(1) as f64;
        let mn = lens.iter().cloned().fold(f64::MAX, f64::min);
        let mx = lens.iter().cloned().fold(0.0, f64::max);
        md.push_str(&format!(
            "- จังหวะตัดต่อ: {} ช็อต · ยาวเฉลี่ย {avg:.1} วินาที (สั้นสุด {mn:.1} · ยาวสุด {mx:.1}) · ตัด {:.0} ครั้ง/นาที\n",
            sh.len(),
            (sh.len().saturating_sub(1)) as f64 * 60.0 / p.duration.max(1.0)
        ));
        let pk = picks(&sh, SHEET_MAX);
        let mut frame_of: BTreeMap<usize, Vec<(String, [u8; 3])>> = BTreeMap::new();
        let mut all = vec![];
        for (k, (shot, t)) in pk.iter().enumerate() {
            let f = format!("f{:02}.jpg", k + 1);
            let fpath = out.join("frames").join(&f);
            let _ = run(Command::new(&ff).args(["-v", "error", "-y", "-ss", &format!("{t:.2}"), "-i"]).arg(media).args(["-frames:v", "1", "-vf", "scale='min(640,iw)':-2", "-q:v", "4"]).arg(&fpath));
            if fpath.is_file() {
                let cs = colours(&ff, &fpath);
                all.extend(cs.iter().cloned());
                frame_of.entry(*shot).or_default().push((format!("{f} ({})", mmss(*t)), mean(&cs)));
            }
        }
        let n = frame_of.values().map(|v| v.len()).sum::<usize>();
        if n > 0 {
            let rows = n.div_ceil(4);
            let _ = run(Command::new(&ff).args(["-v", "error", "-y", "-i"]).arg(out.join("frames").join("f%02d.jpg")).args([
                "-vf",
                &format!("scale=270:270:force_original_aspect_ratio=decrease,pad=270:270:(ow-iw)/2:(oh-ih)/2:color=black,tile=4x{rows}:padding=4:color=white"),
                "-frames:v",
                "1",
                "-q:v",
                "4",
            ]).arg(out.join("sheet.jpg")));
        }
        let pal = palette(&all);
        if !pal.is_empty() {
            md.push_str(&format!("- โทนสีหลัก: {}\n", pal.iter().map(|(h, s)| format!("{h} ({s}%)")).collect::<Vec<_>>().join(" · ")));
        }
        md.push_str(&format!(
            "\n## ภาพรวม\n\n`sheet.jpg` = ภาพจาก {n} จังหวะ เรียงซ้าย→ขวา บน→ล่าง ตามลำดับ f01, f02, … ในตาราง — **ดูภาพนี้ภาพเดียวก่อน** (ราว 1,500 token) แล้วเปิด `frames/fNN.jpg` เฉพาะช็อตที่ต้องดูละเอียด\n\n## ช็อต\n\n| # | ช่วงเวลา | ยาว (วิ) | ภาพ | สีเฉลี่ย | แสง |\n|---|---|---|---|---|---|\n"
        ));
        let show = sh.len().min(80);
        for (i, (a, b)) in sh.iter().enumerate().take(show) {
            let (pic, c) = frame_of.get(&i).and_then(|v| v.first().cloned()).map(|(f, c)| (f, Some(c))).unwrap_or_default();
            let (ch, li) = c.map(|c| (hex(c), light(c).to_string())).unwrap_or_default();
            md.push_str(&format!("| {} | {}–{} | {:.1} | {pic} | {ch} | {li} |\n", i + 1, mmss(*a), mmss(*b), b - a));
        }
        if sh.len() > show {
            md.push_str(&format!("| … | อีก {} ช็อต | | | | |\n", sh.len() - show));
        }
        let all_frames: Vec<String> = frame_of.values().flatten().map(|(f, _)| f.clone()).collect();
        if !all_frames.is_empty() {
            md.push_str(&format!("\nภาพใน sheet.jpg ตามลำดับ: {}\n", all_frames.join(" · ")));
        }
    }

    // sound: loudness now, words later (a speech model takes a while)
    let pcm_path = out.join("audio.f32");
    let mut cuts = vec![];
    if p.audio {
        let _ = run(Command::new(&ff).args(["-v", "error", "-y", "-i"]).arg(media).args(["-vn", "-ac", "1", "-ar", "16000", "-t", &format!("{SPEECH_MAX_SECS}"), "-f", "f32le"]).arg(&pcm_path));
        let pcm = read_f32(&pcm_path);
        let (c, share) = speech_cuts(&pcm);
        cuts = c;
        let peak = pcm.iter().fold(0.0f32, |m, x| m.max(x.abs()));
        let rms = (pcm.iter().map(|x| x * x).sum::<f32>() / pcm.len().max(1) as f32).sqrt();
        md.push_str(&format!(
            "\n## เสียง\n\n- มีเสียงดังต่อเนื่องราว {:.0}% ของคลิป · ความดังเฉลี่ย {:.0} dB · สูงสุด {:.0} dB\n",
            share * 100.0,
            20.0 * rms.max(1e-6).log10(),
            20.0 * peak.max(1e-6).log10()
        ));
    }
    md.push_str(&format!("\n{SPEECH_MARK}\n"));
    let r = recipes(root);
    let chosen = if p.audio && !cuts.is_empty() { choose(root, &r, pref_lang, &pcm_path, &cuts) } else { None };
    if p.audio && chosen.is_none() {
        md.push_str("\n## คำพูด\n\nยังไม่ได้ถอดคำพูด (เครื่องนี้ยังไม่ได้ติดตั้งโมเดลถอดเสียงในส่วนเสริม) — ถ้าคำพูดในคลิปสำคัญกับงาน ให้ขอเจ้าของติดตั้ง\n");
    } else if chosen.is_some() {
        md.push_str("\n## คำพูด\n\nกำลังถอดคำพูด… (แอปจะเติมส่วนนี้เมื่อเสร็จ — ถ้ายังไม่มา ทำส่วนอื่นก่อน)\n");
    }
    md.push_str(FOOTER);
    std::fs::write(out.join("VIDEO.md"), &md)?;

    if let Some((model, eng, lang)) = chosen {
        let speech = match transcribe(root, &eng, &model, &pcm_path, &cuts, &lang, None) {
            Ok(h) => speech_md(&h, &model, &lang),
            Err(e) => format!("\n## คำพูด\n\nถอดคำพูดไม่สำเร็จ: {e:#}\n"),
        };
        let head = md.split(SPEECH_MARK).next().unwrap_or(&md).to_string();
        std::fs::write(out.join("VIDEO.md"), format!("{head}{SPEECH_MARK}\n{speech}{FOOTER}"))?;
    }
    let _ = std::fs::remove_file(&pcm_path);
    Ok(())
}

fn speech_md(h: &[Heard], model: &SpeechModel, lang: &str) -> String {
    let lines: Vec<String> = h.iter().filter(|x| !x.text.trim().is_empty()).map(|x| format!("[{}] {}", mmss_short(x.t0), x.text.trim())).collect();
    if lines.is_empty() {
        return "\n## คำพูด\n\nไม่ได้ยินคำพูดที่ถอดได้ (อาจมีแต่ดนตรีหรือเสียงประกอบ)\n".into();
    }
    let thai = lang == "th" || h.iter().any(|x| x.lang == "th");
    let mut s = format!("\n## คำพูด\n\nถอดด้วยเครื่อง ({}) ในเครื่องนี้ — อาจผิดบางคำ อ่านเอาใจความ\n", model.label);
    if thai {
        s.push_str("\nคำภาษาอังกฤษในคลิปไทยถูกเขียนเป็นตัวไทยตามเสียง (เช่น \"แอปพลิเคชั่น\" \"ไทม์ไลน์\" \"เอไอ\") — เวลานำไปใช้หรืออ้างถึง ให้เขียนเป็น**ภาษาอังกฤษ**สำหรับศัพท์เทคนิค ชื่อแบรนด์/โปรแกรม และคำที่คนไทยนิยมเขียนเป็นอังกฤษ (AI, Excel, TikTok, timeline) แต่คง**คำไทย**ไว้เมื่อคำไทยเข้าใจง่ายกว่า\n");
    }
    s.push('\n');
    s.push_str(&lines.join("\n"));
    s.push('\n');
    s
}

const FOOTER: &str = "\n## ถ้าเจ้าของขอให้ทำตามสไตล์ของคลิปนี้\n\nสรุปเป็น `STYLE.md` ก่อน: โทนสี แสง จังหวะตัดต่อ มุมกล้อง/การเคลื่อนกล้อง การเปลี่ยนฉาก ตัวอักษรบนจอ อารมณ์เสียงและดนตรี โครงเรื่อง (เปิด–กลาง–ปิด) แล้วใช้สูตรนั้นสร้างงานในรูปแบบที่เจ้าของขอ — เลียนแบบ**สไตล์** ไม่คัดลอกเนื้อหา ภาพ หรือคำพูดของคลิป\n";

// ---------------------------------------------------------------------------------------------------------------
// Install / remove (owner presses the button; sizes are shown first)
// ---------------------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize)]
pub struct Job {
    pub part: String,
    /// idle | running | ok | error
    pub state: String,
    pub done_mb: f64,
    pub total_mb: f64,
    pub message: String,
}

static JOB: LazyLock<Mutex<Job>> = LazyLock::new(|| Mutex::new(Job { state: "idle".into(), ..Default::default() }));

pub fn job() -> Job {
    JOB.lock().clone()
}

/// What the settings card shows.
pub fn status(root: &Path) -> Value {
    let r = recipes(root);
    let lang = crate::lang::ui();
    let ff = ffmpeg(root);
    let rec = recommended(&r, &lang).map(|m| m.id).unwrap_or_default();
    let models: Vec<Value> = r
        .speech_models
        .iter()
        .map(|m| {
            let eng = r.engines.iter().find(|e| e.id == m.engine);
            json!({
                "id": m.id, "label": m.label, "note": m.note, "license": m.license, "home": m.home, "size_mb": m.size_mb,
                "installed": model_path(root, m).is_file(),
                "engine": m.engine, "engine_installed": eng.is_some_and(|e| engine_ready(root, e)), "engine_size_mb": eng.map(|e| e.size_mb).unwrap_or(0),
                "score": m.score(&lang), "recommended": m.id == rec,
            })
        })
        .collect();
    json!({
        "lang": lang,
        "ffmpeg": { "found": ff.is_some(), "own": ff.as_ref().is_some_and(|f| f.2), "path": ff.map(|f| f.0.to_string_lossy().into_owned()), "size_mb": r.ffmpeg.size_mb, "label": r.ffmpeg.label },
        "models": models,
        "job": job(),
        "custom_recipes": tools(root).join("recipes.json").is_file(),
    })
}

fn set_job(f: impl FnOnce(&mut Job)) {
    f(&mut JOB.lock());
}

pub(crate) fn job_message(m: String) {
    set_job(|j| {
        j.message = m;
        j.done_mb = 0.0;
        j.total_mb = 0.0;
    });
}

pub(crate) async fn download(url: &str, sha: &str, to: &Path, total_mb: f64) -> Result<()> {
    use sha2::Digest;
    use tokio::io::AsyncWriteExt;
    if let Some(d) = to.parent() {
        std::fs::create_dir_all(d)?;
    }
    let part = to.with_extension("part");
    let client = reqwest::Client::builder().connect_timeout(std::time::Duration::from_secs(30)).build()?;
    let mut resp = client.get(url).send().await?.error_for_status().context("ดาวน์โหลดไม่ได้")?;
    let mut f = tokio::fs::File::create(&part).await?;
    let mut h = sha2::Sha256::new();
    let mut got = 0u64;
    while let Some(chunk) = resp.chunk().await? {
        h.update(&chunk);
        f.write_all(&chunk).await?;
        got += chunk.len() as u64;
        set_job(|j| {
            j.done_mb = got as f64 / 1_048_576.0;
            j.total_mb = total_mb;
        });
    }
    f.flush().await?;
    drop(f);
    let digest = format!("{:x}", h.finalize());
    if !sha.is_empty() && !digest.eq_ignore_ascii_case(sha) {
        let _ = std::fs::remove_file(&part);
        bail!("ไฟล์ที่ได้ไม่ตรงกับที่ควรเป็น (checksum ไม่ตรง) — ไม่ได้ติดตั้ง");
    }
    std::fs::rename(&part, to)?;
    Ok(())
}

/// Unpack an archive with the system's own `tar` (Windows 10+ reads .zip and .tar.gz).
fn unpack(archive: &Path, into: &Path) -> Result<()> {
    std::fs::create_dir_all(into)?;
    let out = run(Command::new("tar").arg("-xf").arg(archive).arg("-C").arg(into))?;
    if !out.status.success() {
        bail!("แตกไฟล์ไม่ได้: {}", String::from_utf8_lossy(&out.stderr).trim());
    }
    let _ = std::fs::remove_file(archive);
    Ok(())
}

pub(crate) async fn fetch_unpacked(url: &str, sha: &str, size_mb: u64, into: &Path) -> Result<()> {
    let name = url.rsplit('/').next().unwrap_or("download").split('?').next().unwrap_or("download").to_string();
    let tmp = into.join(&name);
    let _ = std::fs::remove_dir_all(into);
    download(url, sha, &tmp, size_mb as f64).await?;
    unpack(&tmp, into)
}

/// Install "ffmpeg" or "model:<id>" (its engine comes along when missing).
pub async fn install(root: PathBuf, part: String) -> Result<()> {
    let r = recipes(&root);
    if part == "uv" || part.starts_with("conv:") {
        return crate::docs::install(&root, &part, &r.uv).await;
    }
    if part == "ffmpeg" {
        set_job(|j| j.message = format!("กำลังดาวน์โหลด {}", r.ffmpeg.label));
        return fetch_unpacked(&r.ffmpeg.url, &r.ffmpeg.sha256, r.ffmpeg.size_mb, &tools(&root).join("ffmpeg")).await;
    }
    let id = part.strip_prefix("model:").context("ไม่รู้จักส่วนที่จะติดตั้ง")?;
    let m = r.speech_models.iter().find(|m| m.id == id).context("ไม่รู้จักโมเดลนี้")?.clone();
    let e = r.engines.iter().find(|e| e.id == m.engine).context("ไม่รู้จักเครื่องมือถอดเสียงของโมเดลนี้")?.clone();
    if ffmpeg(&root).is_none() {
        set_job(|j| j.message = format!("กำลังดาวน์โหลด {} (ใช้แยกเสียงจากวิดีโอ)", r.ffmpeg.label));
        fetch_unpacked(&r.ffmpeg.url, &r.ffmpeg.sha256, r.ffmpeg.size_mb, &tools(&root).join("ffmpeg")).await?;
    }
    if !engine_ready(&root, &e) {
        set_job(|j| j.message = format!("กำลังดาวน์โหลด {}", e.label));
        fetch_unpacked(&e.url, &e.sha256, e.size_mb, &engine_dir(&root, &e.id)).await?;
    }
    if !model_path(&root, &m).is_file() {
        set_job(|j| j.message = format!("กำลังดาวน์โหลด {}", m.label));
        download(&m.url, &m.sha256, &model_path(&root, &m), m.size_mb as f64).await?;
    }
    Ok(())
}

pub fn start_install(root: PathBuf, part: String) -> Result<(), String> {
    {
        let mut j = JOB.lock();
        if j.state == "running" {
            return Err("กำลังติดตั้งอยู่ รอให้เสร็จก่อน".into());
        }
        *j = Job { part: part.clone(), state: "running".into(), ..Default::default() };
    }
    tauri::async_runtime::spawn(async move {
        let res = install(root, part).await;
        set_job(|j| match res {
            Ok(()) => {
                j.state = "ok".into();
                j.message = "ติดตั้งเสร็จแล้ว".into();
            }
            Err(e) => {
                j.state = "error".into();
                j.message = format!("{e:#}");
            }
        });
    });
    Ok(())
}

/// Remove "ffmpeg" (only the add-on's own copy) or "model:<id>" (and its engine when no other model needs it).
pub fn remove(root: &Path, part: &str) -> Result<()> {
    let r = recipes(root);
    if part == "uv" || part.starts_with("conv:") {
        return crate::docs::remove(root, part);
    }
    if part == "ffmpeg" {
        let _ = std::fs::remove_dir_all(tools(root).join("ffmpeg"));
        return Ok(());
    }
    let id = part.strip_prefix("model:").context("ไม่รู้จักส่วนที่จะลบ")?;
    let m = r.speech_models.iter().find(|m| m.id == id).context("ไม่รู้จักโมเดลนี้")?;
    let _ = std::fs::remove_file(model_path(root, m));
    let still = r.speech_models.iter().any(|o| o.engine == m.engine && o.id != m.id && model_path(root, o).is_file());
    if !still {
        let _ = std::fs::remove_dir_all(engine_dir(root, &m.engine));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_recipes_read_and_pick_by_language() {
        let r: Recipes = serde_json::from_str(DEFAULT_RECIPES).unwrap();
        assert!(r.version > 0 && !r.ffmpeg.url.is_empty());
        assert_eq!(recommended(&r, "th").unwrap().id, "openthai2-asr");
        assert_eq!(recommended(&r, "en").unwrap().id, "whisper-large-v3-turbo");
        assert_eq!(recommended(&r, "ja").unwrap().id, "whisper-large-v3-turbo");
        for m in &r.speech_models {
            assert!(r.engines.iter().any(|e| e.id == m.engine), "{} has no engine", m.id);
            assert_eq!(m.sha256.len(), 64);
        }
    }

    #[test]
    fn shots_and_picks() {
        let sh = shots(&[0.1, 2.0, 2.1, 5.0], 6.0);
        assert_eq!(sh, vec![(0.0, 2.0), (2.0, 5.0), (5.0, 6.0)]);
        assert_eq!(picks(&sh, 16).len(), 3);
        let many: Vec<(f64, f64)> = (0..40).map(|i| (i as f64, i as f64 + 1.0)).collect();
        let p = picks(&many, 16);
        assert_eq!(p.len(), 16);
        assert_eq!(p[0].0, 0);
        assert_eq!(p[15].0, 39);
        // one long shot still gets several frames
        assert!(picks(&[(0.0, 60.0)], 16).len() >= 6);
    }

    #[test]
    fn speech_is_cut_at_the_quiet_moments() {
        // 30 s: loud, a quiet gap at 12–12.5 s, loud again
        let mut pcm = vec![0.3f32; 16000 * 30];
        for x in &mut pcm[16000 * 12..16000 * 12 + 8000] {
            *x = 0.0005;
        }
        let (cuts, share) = speech_cuts(&pcm);
        assert_eq!(cuts.len(), 2);
        assert!((cuts[0].1 - 12.0).abs() < 0.6, "{cuts:?}");
        assert!(share > 0.9);
        // silence only: nothing to transcribe
        assert!(speech_cuts(&vec![0.0f32; 16000 * 5]).0.is_empty());
    }

    /// Network: download → checksum → unpack, with the small engine archive (17 MB).
    #[tokio::test]
    #[ignore]
    async fn engine_installs_from_its_recipe() {
        let r: Recipes = serde_json::from_str(DEFAULT_RECIPES).unwrap();
        let e = &r.engines[0];
        let root = std::env::temp_dir().join(format!("ia-video-{}", std::process::id()));
        fetch_unpacked(&e.url, &e.sha256, e.size_mb, &engine_dir(&root, &e.id)).await.unwrap();
        assert!(engine_ready(&root, e));
        // a wrong checksum is refused and leaves nothing behind
        let bad = fetch_unpacked(&e.url, &"0".repeat(64), e.size_mb, &root.join("bad")).await;
        assert!(bad.is_err());
        assert!(!root.join("bad").join("transcribe-native-0.4.2-windows-x86_64-cpu-vulkan.tar.gz").exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn cuts_are_clear_peaks_not_motion() {
        let mut sc: Vec<(f64, f64)> = (0..300).map(|i| (i as f64 / 30.0, 0.01)).collect();
        sc[90].1 = 0.4; // a hard cut at 3 s
        for x in &mut sc[150..160] {
            x.1 = 0.05; // motion: busy but no cut
        }
        sc[240].1 = 0.25;
        sc[242].1 = 0.30; // one cut, two frames high
        assert_eq!(cuts_from_scores(&sc), vec![3.0, 242.0 / 30.0]);
    }

    /// A real clip, end to end: VIDEO_TEST_FILE=<video> IA_SPEECH_EXE=<target/release/ia.exe> cargo test --lib -- --ignored digest_real
    #[test]
    #[ignore]
    fn digest_real_file() {
        let f = PathBuf::from(std::env::var("VIDEO_TEST_FILE").unwrap());
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
        let t = std::time::Instant::now();
        digest(&root, &f, "th").unwrap();
        eprintln!("digest took {:?}", t.elapsed());
        eprintln!("{}", std::fs::read_to_string(digest_dir(&f).join("VIDEO.md")).unwrap());
    }

    #[test]
    fn palette_groups_close_colours() {
        let mut all = vec![[250, 250, 250]; 6];
        all.extend(vec![[10, 10, 12]; 3]);
        let p = palette(&all);
        assert_eq!(p[0].1, 67);
        assert_eq!(p.len(), 2);
    }
}
