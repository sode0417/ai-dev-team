-- sprints テーブル (scan_sessions を進化)
CREATE TABLE sprints (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    status TEXT NOT NULL DEFAULT 'selecting',
    -- selecting → hearing → planning → executing → retrospective → completed

    -- Phase 1: 選定 (スキャン結果)
    scan_analysis TEXT,
    priority_actions JSONB,

    -- Phase 3: 計画 (PM Agent による実行順序)
    execution_plan TEXT,

    -- Phase 5: 振り返り
    retrospective TEXT,
    improvement_suggestions JSONB,
    user_feedback TEXT,

    -- エラー
    error_log TEXT,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    started_at TIMESTAMPTZ,        -- executing 開始時刻
    completed_at TIMESTAMPTZ
);

CREATE INDEX idx_sprints_project_id ON sprints(project_id);
CREATE INDEX idx_sprints_status ON sprints(status);

-- tasks テーブルに sprint_id を追加
ALTER TABLE tasks ADD COLUMN sprint_id UUID REFERENCES sprints(id) ON DELETE SET NULL;
CREATE INDEX idx_tasks_sprint_id ON tasks(sprint_id);
