# ai-dev-team

PM Agent 主導の自律型開発チーム管理システム。Web UI からタスクを管理し、claude -p パイプラインで自動実行。

## Tech Stack

- **Backend**: Rust / Axum + SQLx + PostgreSQL
- **Frontend**: Next.js + Tailwind CSS
- **実行**: Claude Code CLI (`claude -p`) パイプライン
- **デプロイ**: Mac mini, Backend:8100 / Frontend:3100
- **Tunnel**: `devteam.sode-ai.com`

## アーキテクチャ

```
Web UI (Next.js :3100)
  ↓ REST API / WebSocket
Backend (Axum :8100)
  ├── Projects / Tasks / Executions CRUD
  ├── claude -p パイプライン (Planner → Coder → Reviewer → Test → PR)
  ├── WebSocket 進捗配信
  └── PostgreSQL (ai_dev_team)
```

## ディレクトリ構成

```
backend/
├── src/
│   ├── main.rs              # Axum server + WebSocket
│   ├── config.rs, db.rs, error.rs, response.rs, auth.rs, ws.rs
│   ├── domains/
│   │   ├── projects/        # handler.rs, model.rs, service.rs
│   │   ├── tasks/           # handler.rs, model.rs, service.rs
│   │   └── executions/      # handler.rs, model.rs, service.rs
│   └── executor/
│       ├── pipeline.rs      # Planner→Coder→Reviewer→Test パイプライン
│       ├── worktree.rs      # git worktree 管理
│       └── claude_cli.rs    # claude CLI ラッパー

frontend/
├── src/app/                 # Next.js App Router
├── src/components/          # 共通コンポーネント
├── src/lib/                 # API クライアント + WebSocket
└── src/types/               # 型定義
```

## コーディング規約

- コミットメッセージは日本語
- Rust: f2a-backend と同じパターン (handler → service → DB)
- Frontend: TypeScript strict, Tailwind CSS

## 環境変数

- `DATABASE_URL` — PostgreSQL 接続文字列 (`postgres://ai_dev_team:...@localhost/ai_dev_team`)
- `PORT` — Backend ポート (デフォルト: 8100)

## 開発

```bash
# Backend
cd backend && cargo run

# Frontend
cd frontend && npm run dev

# DB マイグレーション
psql -U ai_dev_team -d ai_dev_team -f backend/migrations/20260316000000_initial_schema.sql
```

## フェーズ

- **Phase 1 (現在)**: Web UI + 手動タスク作成 + claude -p 実行
- **Phase 2**: PM Agent 自律ループ + チームテンプレート
- **Phase 3**: Claude Code Agent Teams 統合
- **Phase 4**: ai-assistant API 連携
