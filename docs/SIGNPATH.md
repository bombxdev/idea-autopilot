# การสมัครเซ็นชื่อฟรีกับ SignPath Foundation

> **สถานะ 2026-11-04: ถูกปฏิเสธ (ยังไม่ใช่ตอนนี้)** เหตุผลจาก SignPath: โปรเจกต์ยังไม่มีสัญญาณความน่าเชื่อถือจากภายนอกพอ เช่น ดาว/fork/ผู้ร่วมพัฒนาบน GitHub, บทความหรือการพูดถึงจากที่อื่น (Reddit, YouTube ฯลฯ) และกิจกรรมต่อเนื่อง ไม่ใช่เรื่องคุณภาพของงาน และเชิญให้สมัครใหม่ได้
> ทางเลือกแบบเสียเงินของ SignPath ไม่ใช้ (กติกาโปรเจกต์: ฟรีเท่านั้น)
> **สมัครใหม่เมื่อ:** มีผู้ใช้จริงจำนวนหนึ่ง ดาว/fork เพิ่มขึ้น มีคนพูดถึงจากภายนอก (โพสต์, รีวิว, คลิป) และมี release ต่อเนื่องหลายเดือน แล้วใส่ลิงก์เหล่านั้นในช่อง Reputation
> ระหว่างนี้แจกแบบไม่เซ็น: Scoop (ไม่ผ่านเบราว์เซอร์) หรือบอกผู้ใช้ให้กด Unblock / More info → Run anyway

เตรียมไว้ล่วงหน้า ทำได้หลัง repo สาธารณะขึ้น GitHub แล้ว ผมสมัครให้ไม่ได้ เพราะต้องยืนยันตัวตนและยอมรับเงื่อนไขในนามคุณ

## ก่อนสมัคร
1. repo สาธารณะบน GitHub ใช้ GPL-3.0-or-later (มีแล้ว), README อธิบายว่าโปรแกรมทำอะไร (มีแล้ว)
2. เปิด 2FA ที่บัญชี GitHub `bombxdev` (SignPath ต้องการ)
3. มี `docs/CODE_SIGNING_POLICY.md` ใน repo (มีแล้ว) และลิงก์ไปจาก README: เพิ่มบรรทัด “Code signing policy” ใน README หน้าแรกของ repo สาธารณะ
4. ควรมี release แรกบน GitHub (ไฟล์ Setup ที่ยังไม่เซ็น) เพื่อให้เห็นว่าโปรเจกต์ใช้งานจริง

## สมัคร
เข้า <https://signpath.org/apply> ฟอร์มจริงมีมากกว่าที่คิด กรอกตามนี้ (ช่องที่มี * บังคับ)

| ช่อง | กรอกอะไร |
|---|---|
| Project Name* | Idea Autopilot |
| Repository URL* / Homepage URL* | https://github.com/bombxdev/idea-autopilot (ทั้งสองช่อง) |
| Download URL | https://github.com/bombxdev/idea-autopilot/releases (หน้านี้ต้องมีข้อความว่าใช้ SignPath Foundation เซ็นโค้ด: ใส่ไว้ใน README แล้ว และใส่ในคำอธิบาย release ด้วย) |
| Privacy Policy URL | https://github.com/bombxdev/idea-autopilot/blob/main/docs/CODE_SIGNING_POLICY.md (มีหัวข้อ privacy) |
| Wikipedia URL | เว้นว่าง |
| Tagline* | A Windows tray app that lets an AI agent (Claude Code, Codex CLI, Gemini CLI, OpenCode or a custom command) research and build the user's ideas while they are away, within a quota/hour budget. Local only, no telemetry. |
| Category* | AI Tools |
| Description* | Idea Autopilot is a free, open-source (GPL-3.0) Windows tray app. You jot down ideas in one line; while you are away it uses your spare AI-agent quota to research them, ask short questions, and build small prototypes in per-idea folders. It works with any command-line AI agent, keeps all data on your own machine, has no telemetry or accounts, and includes a skill library with a safe, free skill shop that scans skills before install. |
| Reputation* | พูดตรงๆ: ยังใหม่ ไม่มีดาว ให้เขียนตามจริง เช่น "New project (first public release, <วันที่>). Used daily by the author and shared with colleagues; fully open source with CI, tests and documented signing/privacy policy." แล้วใส่ลิงก์ repo, CHANGELOG, หน้า release ห้ามแต่งตัวเลขหรือผู้ใช้ที่ไม่มีจริง |
| Maintainer Type | Individual (ถ้ามีตัวเลือกคล้าย "Individual / Independent developer") |
| Build System* | GitHub Actions (workflow เซ็นจะใช้ตัวนี้) |
| First / Last Name* | ชื่อ-นามสกุลจริงของคุณ (เขียนอังกฤษ) |
| Email* | อีเมลที่รับแจ้งผลได้จริง (ถ้าไม่อยากใช้อีเมลบริษัท ใช้อีเมลส่วนตัว) |
| Company Name | เว้นว่าง (โปรเจกต์ส่วนตัว ไม่ใช่ของบริษัท) |
| Primary Discovery Channel* | เลือกตามจริง เช่น Google search / Other |
| ติ๊ก Code of Conduct* และ personal data* | ต้องติ๊กสองข้อนี้ ส่วน "other communications" ติ๊กหรือไม่ก็ได้ |

ข้อควรรู้: SignPath อาจปฏิเสธโปรเจกต์ที่ยังไม่มีชื่อเสียง ถ้าถูกปฏิเสธไม่ใช่เรื่องผิดปกติ ให้สะสมผู้ใช้/ดาว/release สัก 2-3 เดือนแล้วสมัครใหม่ ระหว่างนี้แจกแบบไม่เซ็นพร้อมวิธีกด "More info → Run anyway" ตามเดิม

## หลังอนุมัติ
1. ใน SignPath สร้าง project + artifact configuration สำหรับไฟล์ `.exe` (ตัวติดตั้ง NSIS และ `idea-autopilot.exe`, `ia.exe`) และ signing policy
2. ใน GitHub: Settings → Secrets → ใส่ `SIGNPATH_API_TOKEN` และตัวแปร `SIGNPATH_ORGANIZATION_ID`
3. คัดลอก `docs/signpath-release.yml.example` ไปไว้ที่ `.github/workflows/release.yml` แล้วแก้ `project-slug`, `signing-policy-slug` ให้ตรงกับที่สร้าง
4. ติดแท็ก `v0.2.2` ฯลฯ → workflow build, ส่งเซ็น, แนบ Setup ที่เซ็นแล้วใน GitHub Release
5. ใน README ใส่เครดิต: “Free code signing provided by SignPath.io, certificate by SignPath Foundation”

หมายเหตุ: ชื่อผู้เผยแพร่ที่ Windows แสดงจะเป็น “SignPath Foundation” ส่วนหน้าเตือน SmartScreen จะลดลงเมื่อมีผู้ติดตั้งมากพอ
