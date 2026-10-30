# การสมัครเซ็นชื่อฟรีกับ SignPath Foundation

เตรียมไว้ล่วงหน้า ทำได้หลัง repo สาธารณะขึ้น GitHub แล้ว ผมสมัครให้ไม่ได้ เพราะต้องยืนยันตัวตนและยอมรับเงื่อนไขในนามคุณ

## ก่อนสมัคร
1. repo สาธารณะบน GitHub ใช้ GPL-3.0-or-later (มีแล้ว), README อธิบายว่าโปรแกรมทำอะไร (มีแล้ว)
2. เปิด 2FA ที่บัญชี GitHub `bombxdev` (SignPath ต้องการ)
3. มี `docs/CODE_SIGNING_POLICY.md` ใน repo (มีแล้ว) และลิงก์ไปจาก README: เพิ่มบรรทัด “Code signing policy” ใน README หน้าแรกของ repo สาธารณะ
4. ควรมี release แรกบน GitHub (ไฟล์ Setup ที่ยังไม่เซ็น) เพื่อให้เห็นว่าโปรเจกต์ใช้งานจริง

## สมัคร
1. เข้า <https://signpath.org/apply> (หน้าสมัครของ SignPath Foundation) แล้วกรอกฟอร์มโปรเจกต์
2. ข้อมูลที่ใช้กรอก
   - ชื่อโปรเจกต์: Idea Autopilot
   - Repo: https://github.com/bombxdev/idea-autopilot
   - License: GPL-3.0-or-later
   - คำอธิบายสั้น: A Windows tray app that lets an AI agent (Claude Code, Codex CLI, Gemini CLI, OpenCode or a custom command) research and build the user's ideas while they are away, within a quota/hour budget. Local only, no telemetry.
   - Code signing policy: https://github.com/bombxdev/idea-autopilot/blob/main/docs/CODE_SIGNING_POLICY.md
   - ผู้ดูแล/ผู้อนุมัติ: bombxdev (คนเดียว)
3. รอการอนุมัติ (เวลาไม่แน่นอน)

## หลังอนุมัติ
1. ใน SignPath สร้าง project + artifact configuration สำหรับไฟล์ `.exe` (ตัวติดตั้ง NSIS และ `idea-autopilot.exe`, `ia.exe`) และ signing policy
2. ใน GitHub: Settings → Secrets → ใส่ `SIGNPATH_API_TOKEN` และตัวแปร `SIGNPATH_ORGANIZATION_ID`
3. คัดลอก `docs/signpath-release.yml.example` ไปไว้ที่ `.github/workflows/release.yml` แล้วแก้ `project-slug`, `signing-policy-slug` ให้ตรงกับที่สร้าง
4. ติดแท็ก `v0.2.2` ฯลฯ → workflow build, ส่งเซ็น, แนบ Setup ที่เซ็นแล้วใน GitHub Release
5. ใน README ใส่เครดิต: “Free code signing provided by SignPath.io, certificate by SignPath Foundation”

หมายเหตุ: ชื่อผู้เผยแพร่ที่ Windows แสดงจะเป็น “SignPath Foundation” ส่วนหน้าเตือน SmartScreen จะลดลงเมื่อมีผู้ติดตั้งมากพอ
