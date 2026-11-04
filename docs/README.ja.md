# Idea Autopilot

[ไทย](../README.md) · [English](../README.md) · [中文](README.zh.md) · **日本語** · [한국어](README.ko.md) · [Tiếng Việt](README.vi.md) · [Bahasa Indonesia](README.id.md) · [Español](README.es.md) · [Français](README.fr.md) · [Deutsch](README.de.md) · [Português](README.pt.md) · [Русский](README.ru.md) · [العربية](README.ar.md) · [हिन्दी](README.hi.md) · [Italiano](README.it.md) · [Türkçe](README.tr.md)

![Idea Autopilot](img/morning.png)

> アイデアを一行メモしておくと、あなたが選んだ AI エージェントが留守中に調査・判断・最初の試作までしてくれます。使うのは本来無駄になる枠だけで、仕事に必要な分には手を付けません。

## 主な機能

- **一行でアイデアを記録**：AI が調べ、判断し、試作まで進めます。
- **どの AI でも**：Claude Code、Codex CLI、Gemini CLI、OpenCode、Mistral Vibe、または自作コマンド。複数アカウント対応。
- **余った枠を活用**：仕事用の枠を先に確保し、勤務時間（カレンダー）を避けて残りだけ使います。
- **難しさに応じてモデルと強度を自動選択**：固定ルールなので選ぶためにトークンを使いません。
- **データは自分の PC だけ**：テレメトリもサーバーもなし。アイデアごとに git リポジトリを作成。
- **スキルライブラリ**：同じ用途の“レシピ”を交代で試して成果を記録。無料スキルショップはインストール前に安全性をスキャン。
- **スマホ**：自分の Telegram ボットで通知とアイデア入力。
- **16 言語の画面**（ほかの言語は自分の AI で翻訳）。

![Skills](img/skills.png)

## インストール（簡単な順）

```
scoop bucket add bomb https://github.com/bombxdev/scoop-bucket
scoop install idea-autopilot
```

Windows 10/11、git、ログイン済みの AI エージェントが 1 つ以上必要です。SmartScreen の警告が出る場合があります（未署名）：詳細情報 → 実行。

[Releases](https://github.com/bombxdev/idea-autopilot/releases) — Setup.exe / portable zip

## 安全性

ボットは各アイデアのフォルダ内だけで動作し、push・公開・デプロイ・アカウント作成・支払いは禁止です。スキルは助言にすぎず、これらのルールを上書きできません。AI が勝手にスキルをインストールすることもできません。

## ライセンス

GPL-3.0-or-later © 2026 bombxdev — [LICENSE](../LICENSE)

[詳細ドキュメント（タイ語/英語）](../README.md)
