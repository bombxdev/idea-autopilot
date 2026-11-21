//! Which programming languages an idea's folder is written in, by size — like the "Languages" bar of a GitHub
//! repository. Counted from file extensions (names and colours as GitHub shows them); prose and data files (Markdown,
//! JSON, YAML…), dependencies and build output are left out, as GitHub does.

use std::collections::HashMap;
use std::path::Path;

/// (extension, language, colour)
const LANGS: &[(&str, &str, &str)] = &[
    ("rs", "Rust", "#dea584"),
    ("svelte", "Svelte", "#ff3e00"),
    ("html", "HTML", "#e34c26"),
    ("htm", "HTML", "#e34c26"),
    ("css", "CSS", "#663399"),
    ("scss", "SCSS", "#c6538c"),
    ("sass", "Sass", "#a53b70"),
    ("less", "Less", "#1d365d"),
    ("js", "JavaScript", "#f1e05a"),
    ("mjs", "JavaScript", "#f1e05a"),
    ("cjs", "JavaScript", "#f1e05a"),
    ("jsx", "JavaScript", "#f1e05a"),
    ("ts", "TypeScript", "#3178c6"),
    ("tsx", "TypeScript", "#3178c6"),
    ("mts", "TypeScript", "#3178c6"),
    ("vue", "Vue", "#41b883"),
    ("py", "Python", "#3572a5"),
    ("java", "Java", "#b07219"),
    ("kt", "Kotlin", "#a97bff"),
    ("kts", "Kotlin", "#a97bff"),
    ("go", "Go", "#00add8"),
    ("c", "C", "#555555"),
    ("h", "C", "#555555"),
    ("cpp", "C++", "#f34b7d"),
    ("cc", "C++", "#f34b7d"),
    ("hpp", "C++", "#f34b7d"),
    ("ino", "C++", "#f34b7d"),
    ("cs", "C#", "#178600"),
    ("ps1", "PowerShell", "#012456"),
    ("psm1", "PowerShell", "#012456"),
    ("sh", "Shell", "#89e051"),
    ("bash", "Shell", "#89e051"),
    ("bat", "Batchfile", "#c1f12e"),
    ("cmd", "Batchfile", "#c1f12e"),
    ("sql", "SQL", "#e38c00"),
    ("php", "PHP", "#4f5d95"),
    ("rb", "Ruby", "#701516"),
    ("swift", "Swift", "#f05138"),
    ("dart", "Dart", "#00b4ab"),
    ("lua", "Lua", "#000080"),
    ("r", "R", "#198ce7"),
    ("ipynb", "Jupyter Notebook", "#da5b0b"),
];

/// folders that are not the idea's own code
const SKIP_DIRS: &[&str] = &[
    ".git", "node_modules", "dist", "build", "target", "out", ".next", ".svelte-kit", ".venv", "venv", "env", "__pycache__",
    "coverage", ".cache", "vendor", "logs", "attachments", "playwright-report", "test-results", "bin", "obj", ".gradle", ".idea", ".vscode",
];

/// a generated file this big is not hand-written code (bundles, minified libraries)
const MAX_FILE: u64 = 2_000_000;
const MAX_FILES: usize = 20_000;

fn lang_of(name: &str) -> Option<(&'static str, &'static str)> {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".min.js") || lower.ends_with(".min.css") || lower.ends_with(".d.ts") {
        return None;
    }
    let ext = lower.rsplit_once('.')?.1;
    LANGS.iter().find(|(e, _, _)| *e == ext).map(|(_, n, c)| (*n, *c))
}

/// Languages of the folder, largest first: [{name, color, bytes, pct}] (pct with one decimal).
pub fn of_dir(dir: &Path) -> Vec<serde_json::Value> {
    let mut sizes: HashMap<&str, (u64, &str)> = HashMap::new();
    let mut stack = vec![dir.to_path_buf()];
    let mut seen = 0usize;
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            seen += 1;
            if seen > MAX_FILES {
                break;
            }
            let name = e.file_name().to_string_lossy().to_string();
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_dir() {
                if !SKIP_DIRS.contains(&name.as_str()) && !name.starts_with('.') {
                    stack.push(e.path());
                }
            } else if ft.is_file() {
                if let Some((lang, color)) = lang_of(&name) {
                    let len = e.metadata().map(|m| m.len()).unwrap_or(0);
                    if len > 0 && len <= MAX_FILE {
                        let s = sizes.entry(lang).or_insert((0, color));
                        s.0 += len;
                    }
                }
            }
        }
    }
    let total: u64 = sizes.values().map(|v| v.0).sum();
    let mut v: Vec<(&str, u64, &str)> = sizes.into_iter().map(|(n, (b, c))| (n, b, c)).collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    v.into_iter()
        .map(|(name, bytes, color)| {
            let pct = if total == 0 { 0.0 } else { (bytes as f64 * 1000.0 / total as f64).round() / 10.0 };
            serde_json::json!({ "name": name, "color": color, "bytes": bytes, "pct": pct })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn languages_are_counted_by_size_and_build_output_is_left_out() {
        let d = std::env::temp_dir().join(format!("ia-langs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("src")).unwrap();
        std::fs::create_dir_all(d.join("node_modules/x")).unwrap();
        std::fs::write(d.join("src/main.rs"), "x".repeat(300)).unwrap();
        std::fs::write(d.join("index.html"), "x".repeat(100)).unwrap();
        std::fs::write(d.join("README.md"), "x".repeat(5000)).unwrap();
        std::fs::write(d.join("lib.min.js"), "x".repeat(5000)).unwrap();
        std::fs::write(d.join("node_modules/x/a.js"), "x".repeat(5000)).unwrap();
        let v = of_dir(&d);
        let _ = std::fs::remove_dir_all(&d);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0]["name"], "Rust");
        assert_eq!(v[0]["pct"], 75.0);
        assert_eq!(v[1]["name"], "HTML");
        assert_eq!(v[1]["color"], "#e34c26");
    }
}
