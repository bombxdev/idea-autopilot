# Idea Autopilot

[ไทย](../README.md) · [English](../README.md) · **中文** · [日本語](README.ja.md) · [한국어](README.ko.md) · [Tiếng Việt](README.vi.md) · [Bahasa Indonesia](README.id.md) · [Español](README.es.md) · [Français](README.fr.md) · [Deutsch](README.de.md) · [Português](README.pt.md) · [Русский](README.ru.md) · [العربية](README.ar.md) · [हिन्दी](README.hi.md) · [Italiano](README.it.md) · [Türkçe](README.tr.md)

![Idea Autopilot](img/morning.png)

> 用一行字记下想法；你选择的 AI 代理会在你离开时研究它、评估它，或做出第一版——用的是本来会浪费的额度，并且永远不动你工作所需的那部分。

## 主要功能

- **一行记下想法**：AI 帮你调研、判断、做原型，你回来就能看到结果。
- **任何 AI 都行**：Claude Code、Codex CLI、Gemini CLI、OpenCode、Mistral Vibe 或你自己的命令，支持多个账号。
- **用掉多余的额度**：先为你的工作留出额度，避开工作时间（工作日历），剩下的才让机器人用。
- **按难度自动选模型和强度**：固定规则，不花额度去“思考”。
- **数据只在你电脑上**：无遥测、无服务器，每个想法一个 git 仓库。
- **技能库**：同一类技能的不同“配方”可轮流尝试并统计效果；免费技能商店会先扫描安全性才允许安装。
- **手机**：通过你自己的 Telegram 机器人收通知并输入想法。
- **界面支持 16 种语言**（其余语言可用你自己的 AI 翻译）。

![Skills](img/skills.png)

## 安装（从最简单的开始）

```
scoop bucket add bomb https://github.com/bombxdev/scoop-bucket
scoop install idea-autopilot
```

需要 Windows 10/11、git 和至少一个已登录的 AI 代理。Windows SmartScreen 可能会警告（尚未签名）：更多信息 → 仍要运行。

[Releases](https://github.com/bombxdev/idea-autopilot/releases) — Setup.exe / portable zip

## 安全

机器人只在每个想法自己的文件夹里工作；禁止 push/发布/部署、禁止注册账号、禁止付款。技能只是建议，不能覆盖这些规则，且 AI 不能自行安装技能。

## 许可证

GPL-3.0-or-later © 2026 bombxdev — [LICENSE](../LICENSE)

[完整文档（泰语/英语）](../README.md)
