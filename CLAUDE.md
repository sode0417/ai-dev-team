# ai-dev-team

PM Agent 主導の自律型開発チーム管理システム。スプリントサイクルでタスクを管理し、claude -p パイプラインで自動実行。

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
  ├── Projects / Sprints / Tasks / Executions CRUD
  ├── PM Agent スキャン → タスク提案
  ├── スプリントサイクル (selecting → hearing → planning → executing → retrospective)
  ├── claude -p パイプライン (Planner → Coder → Reviewer → Test → PR)
  ├── WebSocket 進捗配信
  └── PostgreSQL (ai_dev_team)
```

## スプリントサイクル

```
selecting     → スキャン → タスク候補表示 → ユーザーが選定
hearing       → 各タスクのヒアリング（全タスク ready まで待機）
planning      → PM Agent が実行順序確定 → ユーザー承認
executing     → タスクを順次実行
retrospective → 結果表示 + ユーザー FB → 次スプリントに反映
```

- 1プロジェクト1アクティブスプリント制約
- 手動開始（Web UI）

## ディレクトリ構成

```
backend/
├── src/
│   ├── main.rs              # Axum server + WebSocket
│   ├── config.rs, db.rs, error.rs, response.rs, auth.rs, ws.rs
│   ├── domains/
│   │   ├── projects/        # handler.rs, model.rs, service.rs
│   │   ├── sprints/         # handler.rs, model.rs, service.rs
│   │   ├── tasks/           # handler.rs, model.rs, service.rs
│   │   ├── scans/           # handler.rs, model.rs, service.rs
│   │   └── executions/      # handler.rs, model.rs, service.rs
│   ├── scanner/
│   │   └── analyzer.rs      # PM Agent スキャン + スプリント計画・実行
│   └── executor/
│       ├── pipeline.rs      # Planner→Coder→Reviewer→Test パイプライン
│       ├── worktree.rs      # git worktree 管理
│       └── claude_cli.rs    # claude CLI ラッパー

frontend/
├── src/app/                 # Next.js App Router
├── src/components/          # SprintPanel, HearingPanel, PlanApprovalPanel 等
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
psql -U ai_dev_team -d ai_dev_team -f backend/migrations/20260317000000_scan_sessions.sql
psql -U ai_dev_team -d ai_dev_team -f backend/migrations/20260317100000_hearing_flow.sql
psql -U ai_dev_team -d ai_dev_team -f backend/migrations/20260317200000_sprints.sql
psql -U ai_dev_team -d ai_dev_team -f backend/migrations/20260318000000_qa_phase.sql
psql -U ai_dev_team -d ai_dev_team -f backend/migrations/20260319000000_execution_groups.sql
```

## フェーズ

- **Phase 1**: Web UI + 手動タスク作成 + claude -p 実行 ✅
- **Phase 2**: PM Agent スプリントサイクル + GitHub 連携 ✅
- **Phase 3**: execution_group による並列/直列実行制御 ✅
- **Phase 4 (現在)**: Claude Code Agent Teams 統合
- **Phase 5**: ai-assistant (秘書) API 連携
