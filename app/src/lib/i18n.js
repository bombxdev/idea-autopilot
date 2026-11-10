// Small dictionary for the interface text that follows the language setting. Words are fixed in code, so
// showing them in another language costs no tokens. Each entry lists the 16 languages in LANGS order;
// an unknown language falls back to English. Other screens still use Thai text: to move one over,
// add a key here and call t().
export const LANGS = [
  { code: "th", name: "ไทย", sample: "ภาษาไทย — ตั้งค่าเรียบร้อยแล้ว" },
  { code: "en", name: "English", sample: "English — language set" },
  { code: "zh", name: "中文", sample: "中文 — 语言已设置" },
  { code: "ja", name: "日本語", sample: "日本語 — 言語を設定しました" },
  { code: "ko", name: "한국어", sample: "한국어 — 언어가 설정되었습니다" },
  { code: "vi", name: "Tiếng Việt", sample: "Tiếng Việt — đã đặt ngôn ngữ" },
  { code: "id", name: "Bahasa Indonesia", sample: "Bahasa Indonesia — bahasa telah diatur" },
  { code: "es", name: "Español", sample: "Español — idioma configurado" },
  { code: "fr", name: "Français", sample: "Français — langue définie" },
  { code: "de", name: "Deutsch", sample: "Deutsch — Sprache eingestellt" },
  { code: "pt", name: "Português", sample: "Português — idioma definido" },
  { code: "ru", name: "Русский", sample: "Русский — язык установлен" },
  { code: "ar", name: "العربية", sample: "العربية — تم ضبط اللغة", rtl: true },
  { code: "hi", name: "हिन्दी", sample: "हिन्दी — भाषा सेट हो गई" },
  { code: "it", name: "Italiano", sample: "Italiano — lingua impostata" },
  { code: "tr", name: "Türkçe", sample: "Türkçe — dil ayarlandı" },
];

//        th  en  zh  ja  ko  vi  id  es  fr  de  pt  ru  ar  hi  it  tr
const D = {
  now: ["ตอนนี้", "Right now", "当前", "いま", "지금", "Hiện tại", "Sekarang", "Ahora", "Maintenant", "Gerade", "Agora", "Сейчас", "الآن", "अभी", "Adesso", "Şimdi"],
  working: ["ทำงานอยู่", "Working", "工作中", "作業中", "작업 중", "Đang làm việc", "Sedang bekerja", "Trabajando", "Au travail", "Arbeitet", "Trabalhando", "Работает", "يعمل", "काम कर रहा है", "Al lavoro", "Çalışıyor"],
  thinking: ["กำลังคิด (เงียบมา {t})", "Thinking (quiet for {t})", "正在思考（已静默 {t}）", "考え中（{t} 無言）", "생각 중 ({t} 동안 조용함)", "Đang suy nghĩ (im lặng {t})", "Sedang berpikir (diam {t})", "Pensando (en silencio {t})", "Réflexion (silencieux depuis {t})", "Denkt nach (still seit {t})", "Pensando (em silêncio há {t})", "Думает (тишина {t})", "يفكر (صامت منذ {t})", "सोच रहा है ({t} से शांत)", "Sta pensando (in silenzio da {t})", "Düşünüyor ({t} sessiz)"],
  silent: ["เงียบมานาน {t} — อาจค้าง", "Silent for {t} — may be stuck", "已静默 {t} — 可能卡住", "{t} 無言 — 停止の可能性", "{t} 동안 조용함 — 멈췄을 수 있음", "Im lặng {t} — có thể bị treo", "Diam {t} — mungkin macet", "En silencio {t} — puede estar atascado", "Silencieux depuis {t} — peut-être bloqué", "Seit {t} still — evtl. hängt es", "Em silêncio há {t} — pode ter travado", "Тишина {t} — возможно, завис", "صامت منذ {t} — ربما توقف", "{t} से शांत — अटक सकता है", "In silenzio da {t} — forse bloccato", "{t} sessiz — takılmış olabilir"],
  elapsed: ["ทำมาแล้ว {t}", "Running for {t}", "已运行 {t}", "実行から {t}", "{t} 째 실행 중", "Đã chạy {t}", "Berjalan {t}", "Lleva {t}", "En cours depuis {t}", "Läuft seit {t}", "Rodando há {t}", "Работает {t}", "يعمل منذ {t}", "{t} से चल रहा है", "In corso da {t}", "{t} süredir çalışıyor"],
  step: ["ขั้น {a}/{b}", "Step {a}/{b}", "第 {a}/{b} 步", "ステップ {a}/{b}", "{a}/{b} 단계", "Bước {a}/{b}", "Langkah {a}/{b}", "Paso {a}/{b}", "Étape {a}/{b}", "Schritt {a}/{b}", "Etapa {a}/{b}", "Шаг {a}/{b}", "الخطوة {a}/{b}", "चरण {a}/{b}", "Passaggio {a}/{b}", "Adım {a}/{b}"],
  stop: ["หยุด", "Stop", "停止", "停止", "중지", "Dừng", "Hentikan", "Detener", "Arrêter", "Stopp", "Parar", "Стоп", "إيقاف", "रोकें", "Ferma", "Durdur"],
  used: ["ใช้เวลาของขั้นนี้ {p}% · หยุดเองเมื่อครบ {c}", "{p}% of this step's time · stops itself at {c}", "本步骤已用时 {p}% · 到 {c} 自动停止", "このステップの時間の {p}% · {c} に自動停止", "이 단계 시간의 {p}% · {c}에 자동 중지", "{p}% thời gian của bước này · tự dừng lúc {c}", "{p}% waktu langkah ini · berhenti sendiri pukul {c}", "{p}% del tiempo de este paso · se detiene a las {c}", "{p}% du temps de cette étape · s'arrête à {c}", "{p}% der Zeit dieses Schritts · stoppt um {c}", "{p}% do tempo desta etapa · para às {c}", "{p}% времени шага · остановится в {c}", "{p}% من وقت هذه الخطوة · يتوقف عند {c}", "इस चरण के समय का {p}% · {c} पर अपने आप रुकेगा", "{p}% del tempo di questo passaggio · si ferma alle {c}", "Bu adımın süresinin %{p}'i · {c} saatinde durur"],
  nolog: ["ยังไม่มี log", "No log yet", "暂无日志", "ログはまだありません", "아직 로그 없음", "Chưa có nhật ký", "Belum ada log", "Aún no hay registro", "Pas encore de journal", "Noch kein Protokoll", "Ainda sem registro", "Пока нет журнала", "لا يوجد سجل بعد", "अभी कोई लॉग नहीं", "Nessun registro", "Henüz kayıt yok"],
  hint: [
    "เงียบเกิน 7 นาที: ถ้าเป็นงานคิดนานก็ปล่อยได้ ระบบจะหยุดเองเมื่อครบเวลา แล้วทำต่อรอบหน้า — หรือกด ■ หยุด เพื่อให้ลองใหม่เลย",
    "Quiet for over 7 minutes: if it is a long think, leave it — it stops itself when time is up and continues next round. Or press ■ Stop to retry now.",
    "静默超过 7 分钟：如果只是长时间思考可以不管，时间到会自动停止并在下一轮继续；也可以按 ■ 停止立即重试。",
    "7 分以上無言：長い思考ならそのままで構いません。時間切れで自動停止し、次回続行します。■ 停止で今すぐ再試行もできます。",
    "7분 넘게 조용함: 오래 생각하는 중이면 그대로 두세요. 시간이 다 되면 스스로 멈추고 다음 회차에 이어갑니다. ■ 중지로 바로 재시도할 수도 있습니다.",
    "Im lặng hơn 7 phút: nếu chỉ là suy nghĩ lâu thì cứ để, hết giờ sẽ tự dừng và làm tiếp lượt sau. Hoặc bấm ■ Dừng để thử lại ngay.",
    "Diam lebih dari 7 menit: jika hanya berpikir lama, biarkan saja — berhenti sendiri saat waktu habis dan lanjut putaran berikutnya. Atau tekan ■ Hentikan untuk mencoba lagi sekarang.",
    "Más de 7 minutos en silencio: si solo piensa mucho, déjalo — se detiene solo al acabar el tiempo y sigue en la próxima ronda. O pulsa ■ Detener para reintentar ya.",
    "Silencieux depuis plus de 7 minutes : s'il réfléchit longuement, laissez faire — il s'arrête seul à la fin du temps et reprend au prochain tour. Ou appuyez sur ■ Arrêter pour réessayer.",
    "Seit über 7 Minuten still: Wenn es lange nachdenkt, lass es — es stoppt von selbst, wenn die Zeit um ist, und macht in der nächsten Runde weiter. Oder ■ Stopp drücken, um sofort neu zu starten.",
    "Mais de 7 minutos em silêncio: se está pensando muito, deixe — ele para sozinho quando o tempo acabar e continua na próxima rodada. Ou toque em ■ Parar para tentar de novo.",
    "Больше 7 минут тишины: если это долгое размышление, оставьте — по истечении времени остановится само и продолжит в следующем раунде. Или нажмите ■ Стоп, чтобы повторить сразу.",
    "صامت لأكثر من 7 دقائق: إن كان يفكر طويلًا فاتركه — سيتوقف تلقائيًا عند انتهاء الوقت ويكمل في الجولة التالية. أو اضغط ■ إيقاف لإعادة المحاولة الآن.",
    "7 मिनट से ज़्यादा शांत: अगर यह लंबी सोच है तो छोड़ दें — समय पूरा होने पर अपने आप रुकेगा और अगली बारी में जारी रखेगा। या अभी दोबारा कोशिश के लिए ■ रोकें दबाएँ।",
    "In silenzio da oltre 7 minuti: se sta riflettendo a lungo, lascialo — si ferma da solo a fine tempo e riprende al prossimo giro. Oppure premi ■ Ferma per riprovare subito.",
    "7 dakikadan uzun süredir sessiz: uzun düşünüyorsa bırakın — süre dolunca kendiliğinden durur ve sonraki turda devam eder. Ya da hemen yeniden denemek için ■ Durdur'a basın.",
  ],
  sec: ["{n} วินาที", "{n} s", "{n} 秒", "{n} 秒", "{n}초", "{n} giây", "{n} dtk", "{n} s", "{n} s", "{n} s", "{n} s", "{n} с", "{n} ث", "{n} से", "{n} s", "{n} sn"],
  min: ["{n} นาที", "{n} min", "{n} 分钟", "{n} 分", "{n}분", "{n} phút", "{n} mnt", "{n} min", "{n} min", "{n} Min.", "{n} min", "{n} мин", "{n} د", "{n} मि", "{n} min", "{n} dk"],
  hrmin: ["{h} ชม. {m} นาที", "{h} h {m} min", "{h} 小时 {m} 分钟", "{h} 時間 {m} 分", "{h}시간 {m}분", "{h} giờ {m} phút", "{h} jam {m} mnt", "{h} h {m} min", "{h} h {m} min", "{h} Std. {m} Min.", "{h} h {m} min", "{h} ч {m} мин", "{h} س {m} د", "{h} घं {m} मि", "{h} h {m} min", "{h} sa {m} dk"],
  // navigation
  nav_morning: ["เช้านี้", "This morning", "今晨", "今朝", "오늘 아침", "Sáng nay", "Pagi ini", "Esta mañana", "Ce matin", "Heute früh", "Esta manhã", "Утро", "هذا الصباح", "आज सुबह", "Stamattina", "Bu sabah"],
  nav_board: ["บอร์ด", "Board", "看板", "ボード", "보드", "Bảng", "Papan", "Tablero", "Tableau", "Board", "Quadro", "Доска", "اللوحة", "बोर्ड", "Bacheca", "Pano"],
  nav_quota: ["โควต้า", "Quota", "额度", "クォータ", "할당량", "Hạn mức", "Kuota", "Cuota", "Quota", "Kontingent", "Cota", "Квота", "الحصة", "कोटा", "Quota", "Kota"],
  nav_library: ["คลังความรู้", "Library", "知识库", "ライブラリ", "라이브러리", "Thư viện", "Pustaka", "Biblioteca", "Bibliothèque", "Bibliothek", "Biblioteca", "Библиотека", "المكتبة", "लाइब्रेरी", "Libreria", "Kütüphane"],
  nav_skills: ["สกิล", "Skills", "技能", "スキル", "스킬", "Kỹ năng", "Skill", "Habilidades", "Compétences", "Skills", "Habilidades", "Навыки", "المهارات", "स्किल्स", "Competenze", "Beceriler"],
  nav_activity: ["บันทึก", "Activity", "记录", "履歴", "기록", "Nhật ký", "Aktivitas", "Actividad", "Activité", "Aktivität", "Atividade", "Журнал", "النشاط", "गतिविधि", "Attività", "Etkinlik"],
  nav_sessions: ["เซสชัน", "Sessions", "会话", "セッション", "세션", "Phiên", "Sesi", "Sesiones", "Sessions", "Sitzungen", "Sessões", "Сессии", "الجلسات", "सत्र", "Sessioni", "Oturumlar"],
  nav_settings: ["ตั้งค่า", "Settings", "设置", "設定", "설정", "Cài đặt", "Pengaturan", "Ajustes", "Réglages", "Einstellungen", "Configurações", "Настройки", "الإعدادات", "सेटिंग्स", "Impostazioni", "Ayarlar"],
  // language card
  lang_title: ["ภาษา", "Language", "语言", "言語", "언어", "Ngôn ngữ", "Bahasa", "Idioma", "Langue", "Sprache", "Idioma", "Язык", "اللغة", "भाषा", "Lingua", "Dil"],
  lang_desc: [
    "เลือกภาษาที่ใช้ในแผงสถานะ ข้อความความคืบหน้า และที่ AI รายงานกลับมา บันทึกให้อัตโนมัติเมื่อกดเลือก",
    "Choose the language for the status panel, progress messages and what the AI reports back. Saved automatically when you pick.",
    "选择状态面板、进度消息以及 AI 报告所用的语言。选择后自动保存。",
    "ステータス表示、進捗メッセージ、AI の報告に使う言語を選びます。選ぶと自動で保存されます。",
    "상태 패널, 진행 메시지, AI 보고에 사용할 언어를 선택하세요. 선택하면 자동 저장됩니다.",
    "Chọn ngôn ngữ cho bảng trạng thái, thông báo tiến độ và báo cáo của AI. Tự động lưu khi bạn chọn.",
    "Pilih bahasa untuk panel status, pesan kemajuan, dan laporan AI. Tersimpan otomatis saat dipilih.",
    "Elige el idioma del panel de estado, los mensajes de progreso y los informes de la IA. Se guarda solo al elegir.",
    "Choisissez la langue du panneau d'état, des messages de progression et des rapports de l'IA. Enregistré automatiquement.",
    "Wähle die Sprache für Statusfenster, Fortschrittsmeldungen und Berichte der KI. Wird automatisch gespeichert.",
    "Escolha o idioma do painel de status, das mensagens de progresso e dos relatórios da IA. Salvo automaticamente.",
    "Выберите язык панели состояния, сообщений о ходе работы и отчётов ИИ. Сохраняется автоматически.",
    "اختر لغة لوحة الحالة ورسائل التقدّم وتقارير الذكاء الاصطناعي. يُحفظ تلقائيًا عند الاختيار.",
    "स्टेटस पैनल, प्रगति संदेशों और AI की रिपोर्ट के लिए भाषा चुनें। चुनते ही अपने आप सेव हो जाती है।",
    "Scegli la lingua del pannello di stato, dei messaggi di avanzamento e dei rapporti dell'IA. Salvato automaticamente.",
    "Durum paneli, ilerleme mesajları ve yapay zekâ raporları için dili seçin. Seçince otomatik kaydedilir.",
  ],
  lang_follow: ["ตอบตามภาษาที่ฉันพิมพ์", "Reply in the language I type", "按我输入的语言回复", "入力した言語で返信", "입력한 언어로 답변", "Trả lời theo ngôn ngữ tôi gõ", "Balas dalam bahasa yang saya ketik", "Responder en el idioma que escribo", "Répondre dans la langue que je tape", "In der getippten Sprache antworten", "Responder no idioma que eu digito", "Отвечать на языке ввода", "الرد بلغة ما أكتبه", "मैं जिस भाषा में लिखूँ उसी में जवाब", "Rispondi nella lingua che scrivo", "Yazdığım dilde yanıtla"],
  lang_saved: ["บันทึกแล้ว", "Saved", "已保存", "保存しました", "저장됨", "Đã lưu", "Tersimpan", "Guardado", "Enregistré", "Gespeichert", "Salvo", "Сохранено", "تم الحفظ", "सेव हो गया", "Salvato", "Kaydedildi"],
};

const ORDER = LANGS.map((l) => l.code);

export function t(key, lang, vars = {}) {
  const row = D[key];
  if (!row) return key;
  const i = ORDER.indexOf(lang);
  const s = row[i < 0 ? 1 : i] ?? row[1];
  return s.replace(/\{(\w+)\}/g, (_, k) => vars[k] ?? "");
}

/** Seconds as a short phrase in `lang`. */
export function durIn(sec, lang) {
  sec = Math.max(0, Math.round(sec));
  if (sec < 60) return t("sec", lang, { n: sec });
  if (sec < 3600) return t("min", lang, { n: Math.floor(sec / 60) });
  return t("hrmin", lang, { h: Math.floor(sec / 3600), m: Math.floor((sec % 3600) / 60) });
}

/** BCP 47 locale for dates and times in the chosen language. */
const LOCALES = { th: "th-TH", en: "en-GB", zh: "zh-CN", ja: "ja-JP", ko: "ko-KR", vi: "vi-VN", id: "id-ID", es: "es-ES", fr: "fr-FR", de: "de-DE", pt: "pt-BR", ru: "ru-RU", ar: "ar", hi: "hi-IN", it: "it-IT", tr: "tr-TR" };
export const locale = (lang) => LOCALES[lang] ?? "en-GB";

export const isRtl = (lang) => !!LANGS.find((l) => l.code === lang)?.rtl;
