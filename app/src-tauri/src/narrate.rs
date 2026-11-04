//! Turns Claude's tool calls and messages into short plain lines in the reader's language, with
//! rules only (no tokens): "แก้ไฟล์ app.js" instead of `› Edit D:\...\app.js`. Commands are described
//! by the `description` Claude already writes for them; raw code is never shown. The AI's own
//! free text is shown only when it is already in the reader's language, otherwise a short phrase
//! stands in for it (see lang.rs) - nothing is ever sent to a translator.
use serde_json::Value;

/// Phrases per language, in the order of `crate::lang::SUPPORTED`: th en zh ja ko vi id es.
/// `{}` is filled with the detail (file name, query, ...). Unknown language = English.
const T: &[(&str, [&str; 8])] = &[
    ("read", ["📖 อ่านไฟล์ {}", "📖 Reading {}", "📖 读取文件 {}", "📖 ファイルを読み込み {}", "📖 파일 읽기 {}", "📖 Đọc tệp {}", "📖 Membaca berkas {}", "📖 Leyendo {}"]),
    ("edit", ["✏️ แก้ไฟล์ {}", "✏️ Editing {}", "✏️ 修改文件 {}", "✏️ ファイルを編集 {}", "✏️ 파일 수정 {}", "✏️ Sửa tệp {}", "✏️ Mengedit {}", "✏️ Editando {}"]),
    ("write", ["📝 เขียนไฟล์ {}", "📝 Writing {}", "📝 写入文件 {}", "📝 ファイルを作成 {}", "📝 파일 작성 {}", "📝 Ghi tệp {}", "📝 Menulis {}", "📝 Escribiendo {}"]),
    ("notebook", ["✏️ แก้ notebook {}", "✏️ Editing notebook {}", "✏️ 修改笔记本 {}", "✏️ ノートブックを編集 {}", "✏️ 노트북 수정 {}", "✏️ Sửa notebook {}", "✏️ Mengedit notebook {}", "✏️ Editando notebook {}"]),
    ("shell", ["⚙️ รันคำสั่งในเครื่อง", "⚙️ Running a command on this machine", "⚙️ 在本机运行命令", "⚙️ コマンドを実行中", "⚙️ 명령 실행 중", "⚙️ Chạy lệnh trên máy", "⚙️ Menjalankan perintah di komputer", "⚙️ Ejecutando un comando en el equipo"]),
    ("grep", ["🔎 ค้นหา “{}” ในโค้ด", "🔎 Searching the code for “{}”", "🔎 在代码中搜索“{}”", "🔎 コード内を検索「{}」", "🔎 코드에서 “{}” 검색", "🔎 Tìm “{}” trong mã", "🔎 Mencari “{}” di kode", "🔎 Buscando “{}” en el código"]),
    ("glob", ["🗂 หาไฟล์ที่เกี่ยวข้อง", "🗂 Finding related files", "🗂 查找相关文件", "🗂 関連ファイルを探索", "🗂 관련 파일 찾기", "🗂 Tìm các tệp liên quan", "🗂 Mencari berkas terkait", "🗂 Buscando archivos relacionados"]),
    ("web_search", ["🌐 ค้นเว็บ: {}", "🌐 Searching the web: {}", "🌐 搜索网络：{}", "🌐 Web検索：{}", "🌐 웹 검색: {}", "🌐 Tìm trên web: {}", "🌐 Mencari di web: {}", "🌐 Buscando en la web: {}"]),
    ("web_fetch", ["🌐 อ่านหน้าเว็บ {}", "🌐 Reading the page {}", "🌐 阅读网页 {}", "🌐 ページを読み込み {}", "🌐 웹페이지 읽기 {}", "🌐 Đọc trang {}", "🌐 Membaca halaman {}", "🌐 Leyendo la página {}"]),
    ("agent", ["🧑‍💻 ให้ผู้ช่วยไปทำ: {}", "🧑‍💻 Handing off: {}", "🧑‍💻 交给助手处理：{}", "🧑‍💻 アシスタントに依頼：{}", "🧑‍💻 도우미에게 맡김: {}", "🧑‍💻 Giao cho trợ lý: {}", "🧑‍💻 Menyerahkan ke asisten: {}", "🧑‍💻 Delegando: {}"]),
    ("todo_now", ["📋 ขั้นตอนตอนนี้: {}", "📋 Current step: {}", "📋 当前步骤：{}", "📋 現在のステップ：{}", "📋 현재 단계: {}", "📋 Bước hiện tại: {}", "📋 Langkah saat ini: {}", "📋 Paso actual: {}"]),
    ("todo_plan", ["📋 วางลำดับงาน", "📋 Planning the steps", "📋 规划步骤", "📋 手順を計画中", "📋 작업 순서 계획", "📋 Lập kế hoạch các bước", "📋 Menyusun langkah kerja", "📋 Planificando los pasos"]),
    ("mcp", ["🔌 ใช้เครื่องมือ {}", "🔌 Using the tool {}", "🔌 使用工具 {}", "🔌 ツールを使用 {}", "🔌 도구 사용 {}", "🔌 Dùng công cụ {}", "🔌 Memakai alat {}", "🔌 Usando la herramienta {}"]),
    ("tool", ["🛠 ใช้เครื่องมือ {}", "🛠 Using the tool {}", "🛠 使用工具 {}", "🛠 ツールを使用 {}", "🛠 도구 사용 {}", "🛠 Dùng công cụ {}", "🛠 Memakai alat {}", "🛠 Usando la herramienta {}"]),
    // stand in for the AI's thinking / commentary when that is not in the reader's language
    ("thinking", ["🤔 กำลังคิดวิเคราะห์…", "🤔 Thinking it through…", "🤔 正在思考分析…", "🤔 考え中…", "🤔 생각하며 분석 중…", "🤔 Đang suy nghĩ, phân tích…", "🤔 Sedang berpikir dan menganalisis…", "🤔 Pensando y analizando…"]),
    ("saying", ["💬 กำลังสรุปความคืบหน้า…", "💬 Summing up the progress…", "💬 正在总结进展…", "💬 進捗をまとめ中…", "💬 진행 상황 정리 중…", "💬 Đang tóm tắt tiến độ…", "💬 Merangkum kemajuan…", "💬 Resumiendo el avance…"]),
    ("you", ["🙋 คุณ: {}", "🙋 You: {}", "🙋 你：{}", "🙋 あなた：{}", "🙋 나: {}", "🙋 Bạn: {}", "🙋 Anda: {}", "🙋 Tú: {}"]),
    ("error", ["⚠️ เจอปัญหาระหว่างทำ — กำลังหาทางแก้", "⚠️ Hit a problem — working on a fix", "⚠️ 遇到问题，正在设法解决", "⚠️ 問題が発生 — 解決策を探しています", "⚠️ 문제 발생 — 해결 방법을 찾는 중", "⚠️ Gặp sự cố — đang tìm cách xử lý", "⚠️ Ada masalah — sedang dicari solusinya", "⚠️ Hubo un problema — buscando solución"]),
    ("stage_done", ["✓ จบขั้น ({} turns)", "✓ Step finished ({} turns)", "✓ 步骤完成（{} 轮）", "✓ ステップ完了（{} ターン）", "✓ 단계 완료 ({} 턴)", "✓ Xong bước ({} lượt)", "✓ Langkah selesai ({} giliran)", "✓ Paso terminado ({} turnos)"]),
];


/// Phrases for the second group of languages, in the order of `crate::lang::SUPPORTED[8..]`: fr de pt ru ar hi it tr.
const T2: &[(&str, [&str; 8])] = &[
    ("read", ["📖 Lecture de {}", "📖 Lese {}", "📖 Lendo {}", "📖 Чтение {}", "📖 قراءة {}", "📖 पढ़ रहा है: {}", "📖 Lettura di {}", "📖 {} okunuyor"]),
    ("edit", ["✏️ Modification de {}", "✏️ Bearbeite {}", "✏️ Editando {}", "✏️ Правка {}", "✏️ تعديل {}", "✏️ बदल रहा है: {}", "✏️ Modifica di {}", "✏️ {} düzenleniyor"]),
    ("write", ["📝 Écriture de {}", "📝 Schreibe {}", "📝 Escrevendo {}", "📝 Запись {}", "📝 كتابة {}", "📝 लिख रहा है: {}", "📝 Scrittura di {}", "📝 {} yazılıyor"]),
    ("notebook", ["✏️ Modification du notebook {}", "✏️ Bearbeite Notebook {}", "✏️ Editando o notebook {}", "✏️ Правка ноутбука {}", "✏️ تعديل الدفتر {}", "✏️ नोटबुक बदल रहा है: {}", "✏️ Modifica del notebook {}", "✏️ {} not defteri düzenleniyor"]),
    ("shell", ["⚙️ Exécution d'une commande sur cette machine", "⚙️ Führe einen Befehl auf diesem Rechner aus", "⚙️ Executando um comando nesta máquina", "⚙️ Выполнение команды на этом компьютере", "⚙️ تشغيل أمر على هذا الجهاز", "⚙️ इस मशीन पर कमांड चला रहा है", "⚙️ Esecuzione di un comando su questo computer", "⚙️ Bu bilgisayarda bir komut çalıştırılıyor"]),
    ("grep", ["🔎 Recherche de « {} » dans le code", "🔎 Suche „{}“ im Code", "🔎 Procurando “{}” no código", "🔎 Поиск «{}» в коде", "🔎 البحث عن «{}» في الشيفرة", "🔎 कोड में “{}” खोज रहा है", "🔎 Ricerca di «{}» nel codice", "🔎 Kodda “{}” aranıyor"]),
    ("glob", ["🗂 Recherche de fichiers liés", "🗂 Suche passende Dateien", "🗂 Procurando arquivos relacionados", "🗂 Поиск связанных файлов", "🗂 البحث عن الملفات ذات الصلة", "🗂 संबंधित फ़ाइलें खोज रहा है", "🗂 Ricerca di file correlati", "🗂 İlgili dosyalar aranıyor"]),
    ("web_search", ["🌐 Recherche sur le web : {}", "🌐 Websuche: {}", "🌐 Pesquisando na web: {}", "🌐 Поиск в интернете: {}", "🌐 بحث في الويب: {}", "🌐 वेब पर खोज: {}", "🌐 Ricerca sul web: {}", "🌐 Web'de aranıyor: {}"]),
    ("web_fetch", ["🌐 Lecture de la page {}", "🌐 Lese die Seite {}", "🌐 Lendo a página {}", "🌐 Чтение страницы {}", "🌐 قراءة الصفحة {}", "🌐 पेज पढ़ रहा है: {}", "🌐 Lettura della pagina {}", "🌐 {} sayfası okunuyor"]),
    ("agent", ["🧑‍💻 Délégation : {}", "🧑‍💻 Übergebe an Assistenten: {}", "🧑‍💻 Delegando: {}", "🧑‍💻 Поручено помощнику: {}", "🧑‍💻 إسناد إلى مساعد: {}", "🧑‍💻 सहायक को सौंपा: {}", "🧑‍💻 Delega all'assistente: {}", "🧑‍💻 Yardımcıya devrediliyor: {}"]),
    ("todo_now", ["📋 Étape en cours : {}", "📋 Aktueller Schritt: {}", "📋 Etapa atual: {}", "📋 Текущий шаг: {}", "📋 الخطوة الحالية: {}", "📋 वर्तमान चरण: {}", "📋 Passaggio attuale: {}", "📋 Geçerli adım: {}"]),
    ("todo_plan", ["📋 Planification des étapes", "📋 Plane die Schritte", "📋 Planejando as etapas", "📋 Планирование шагов", "📋 تخطيط الخطوات", "📋 चरणों की योजना बना रहा है", "📋 Pianificazione dei passaggi", "📋 Adımlar planlanıyor"]),
    ("mcp", ["🔌 Utilise l'outil {}", "🔌 Nutze das Werkzeug {}", "🔌 Usando a ferramenta {}", "🔌 Использует инструмент {}", "🔌 استخدام الأداة {}", "🔌 टूल का उपयोग: {}", "🔌 Uso dello strumento {}", "🔌 {} aracı kullanılıyor"]),
    ("tool", ["🛠 Utilise l'outil {}", "🛠 Nutze das Werkzeug {}", "🛠 Usando a ferramenta {}", "🛠 Использует инструмент {}", "🛠 استخدام الأداة {}", "🛠 टूल का उपयोग: {}", "🛠 Uso dello strumento {}", "🛠 {} aracı kullanılıyor"]),
    ("thinking", ["🤔 Réflexion en cours…", "🤔 Denke nach…", "🤔 Pensando…", "🤔 Размышляет…", "🤔 يفكر ويحلل…", "🤔 सोच-विचार कर रहा है…", "🤔 Sta ragionando…", "🤔 Düşünüyor…"]),
    ("saying", ["💬 Résumé de l'avancement…", "💬 Fasse den Fortschritt zusammen…", "💬 Resumindo o andamento…", "💬 Подводит итоги…", "💬 تلخيص التقدّم…", "💬 प्रगति का सार बना रहा है…", "💬 Riassume l'avanzamento…", "💬 İlerleme özetleniyor…"]),
    ("you", ["🙋 Vous : {}", "🙋 Du: {}", "🙋 Você: {}", "🙋 Вы: {}", "🙋 أنت: {}", "🙋 आप: {}", "🙋 Tu: {}", "🙋 Siz: {}"]),
    ("error", ["⚠️ Problème rencontré — recherche d'une solution", "⚠️ Problem aufgetreten — suche eine Lösung", "⚠️ Houve um problema — buscando solução", "⚠️ Возникла проблема — ищу решение", "⚠️ حدثت مشكلة — جارٍ البحث عن حل", "⚠️ समस्या आई — समाधान खोज रहा है", "⚠️ Problema riscontrato — cerco una soluzione", "⚠️ Bir sorun çıktı — çözüm aranıyor"]),
    ("stage_done", ["✓ Étape terminée ({} tours)", "✓ Schritt abgeschlossen ({} Runden)", "✓ Etapa concluída ({} turnos)", "✓ Шаг завершён ({} ходов)", "✓ اكتملت الخطوة ({} جولات)", "✓ चरण पूरा ({} बारी)", "✓ Passaggio completato ({} turni)", "✓ Adım tamamlandı ({} tur)"]),
];

/// A phrase in `lang`, with `{}` replaced by `arg`.
pub fn tr(lang: &str, key: &str, arg: &str) -> String {
    let i = crate::lang::SUPPORTED.iter().position(|l| *l == lang).unwrap_or(1);
    let (table, col) = if i < 8 { (T, i) } else { (T2, i - 8) };
    table.iter().find(|(k, _)| *k == key).map(|(_, v)| v[col].replace("{}", arg)).unwrap_or_default()
}

fn file_name(input: &Value, key: &str) -> String {
    input
        .get(key)
        .and_then(Value::as_str)
        .map(|p| p.rsplit(['/', '\\']).next().unwrap_or(p).to_string())
        .unwrap_or_default()
}

fn short(s: &str, n: usize) -> String {
    let one: String = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if one.chars().count() > n {
        format!("{}…", one.chars().take(n).collect::<String>())
    } else {
        one
    }
}

fn host(url: &str) -> String {
    url.split("//").nth(1).unwrap_or(url).split('/').next().unwrap_or(url).trim_start_matches("www.").to_string()
}

/// One line for a tool call, or None for tools not worth showing.
pub fn tool(name: &str, input: &Value, lang: &str) -> Option<String> {
    let s = |k: &str| input.get(k).and_then(Value::as_str).unwrap_or("");
    let line = match name {
        "Read" => tr(lang, "read", &file_name(input, "file_path")),
        "Edit" | "MultiEdit" => tr(lang, "edit", &file_name(input, "file_path")),
        "Write" => tr(lang, "write", &file_name(input, "file_path")),
        "NotebookEdit" => tr(lang, "notebook", &file_name(input, "notebook_path")),
        "Bash" | "PowerShell" => match s("description") {
            // the AI is told to write this in the reader's language; if it did not, a plain phrase stands in
            d if !d.is_empty() && crate::lang::is_in(d, lang) => format!("⚙️ {}", short(d, 120)),
            _ => tr(lang, "shell", ""),
        },
        "Grep" => tr(lang, "grep", &short(s("pattern"), 40)),
        "Glob" => tr(lang, "glob", ""),
        "WebSearch" => tr(lang, "web_search", &short(s("query"), 100)),
        "WebFetch" => tr(lang, "web_fetch", &host(s("url"))),
        "Task" | "Agent" => tr(lang, "agent", &short(s("description"), 80)),
        "TodoWrite" => {
            let now = input.get("todos").and_then(Value::as_array).and_then(|t| {
                t.iter().find(|x| x.get("status").and_then(Value::as_str) == Some("in_progress"))
            });
            match now.and_then(|x| x.get("activeForm").or(x.get("content"))).and_then(Value::as_str) {
                Some(t) if crate::lang::is_in(t, lang) => tr(lang, "todo_now", &short(t, 100)),
                _ => tr(lang, "todo_plan", ""),
            }
        }
        "ToolSearch" | "TaskOutput" | "BashOutput" | "KillShell" | "Monitor" => return None,
        n if n.starts_with("mcp__") => tr(lang, "mcp", &n.rsplit("__").next().unwrap_or(n).replace('_', " ")),
        n => tr(lang, "tool", n),
    };
    Some(line)
}

/// Lines for one assistant message's content blocks (text the AI says + its tool calls).
pub fn assistant(parts: &[Value], lang: &str) -> Vec<String> {
    let mut out = Vec::new();
    for p in parts {
        match p.get("type").and_then(Value::as_str) {
            Some("text") => {
                let t = p.get("text").and_then(Value::as_str).unwrap_or("").trim();
                if !t.is_empty() {
                    out.push(if crate::lang::is_in(t, lang) { format!("💬 {}", short(t, 400)) } else { tr(lang, "saying", "") });
                }
            }
            Some("thinking") => {
                let t = p.get("thinking").and_then(Value::as_str).unwrap_or("").trim();
                if !t.is_empty() {
                    out.push(if crate::lang::is_in(t, lang) { format!("🤔 {}", short(t, 240)) } else { tr(lang, "thinking", "") });
                }
            }
            Some("tool_use") => {
                let name = p.get("name").and_then(Value::as_str).unwrap_or("?");
                if let Some(l) = tool(name, p.get("input").unwrap_or(&Value::Null), lang) {
                    out.push(l);
                }
            }
            _ => {}
        }
    }
    out
}

/// What the owner typed (from the app, the phone or the Claude app). None for tool results and
/// system/meta messages.
pub fn user(v: &Value, lang: &str) -> Option<String> {
    if v.get("isMeta").and_then(Value::as_bool) == Some(true) {
        return None;
    }
    let c = v.pointer("/message/content")?;
    let text = match c {
        Value::String(s) => s.clone(),
        Value::Array(a) => a
            .iter()
            .filter(|p| p.get("type").and_then(Value::as_str) == Some("text"))
            .filter_map(|p| p.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join(" "),
        _ => return None,
    };
    let t = text.trim();
    if t.is_empty() || t.starts_with('<') || t.starts_with("[Request interrupted") {
        return None;
    }
    Some(tr(lang, "you", &short(t, 300)))
}

/// A failed tool call shows up as a tool_result with is_error.
pub fn had_error(v: &Value) -> bool {
    v.pointer("/message/content").and_then(Value::as_array).is_some_and(|a| {
        a.iter().any(|p| p.get("type").and_then(Value::as_str) == Some("tool_result") && p.get("is_error").and_then(Value::as_bool) == Some(true))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn hides_code() {
        let d = json!({ "command": "cd x && npm test", "description": "Run the test suite" });
        assert_eq!(tool("Bash", &d, "en").unwrap(), "⚙️ Run the test suite");
        // an English description is not shown to a Thai reader; a plain phrase stands in
        assert_eq!(tool("Bash", &d, "th").unwrap(), "⚙️ รันคำสั่งในเครื่อง");
        let e = json!({ "file_path": "D:\\a\\src\\app.js" });
        assert_eq!(tool("Edit", &e, "th").unwrap(), "✏️ แก้ไฟล์ app.js");
        assert_eq!(tool("Edit", &e, "ja").unwrap(), "✏️ ファイルを編集 app.js");
        assert!(tool("ToolSearch", &json!({}), "th").is_none());
    }

    #[test]
    fn foreign_thinking_is_replaced() {
        let parts = [json!({ "type": "thinking", "thinking": "The user wants a coffee tracker, let me search for competitors first" })];
        assert_eq!(assistant(&parts, "th"), vec!["🤔 กำลังคิดวิเคราะห์…".to_string()]);
        assert!(assistant(&parts, "en")[0].starts_with("🤔 The user"));
    }

    #[test]
    fn owner_text_only() {
        assert_eq!(user(&json!({ "message": { "content": "แก้ปุ่มให้ใหญ่ขึ้น" } }), "th").unwrap(), "🙋 คุณ: แก้ปุ่มให้ใหญ่ขึ้น");
        assert!(user(&json!({ "message": { "content": [{ "type": "tool_result", "content": "ok" }] } }), "th").is_none());
        assert!(user(&json!({ "isMeta": true, "message": { "content": "x" } }), "th").is_none());
    }
}
