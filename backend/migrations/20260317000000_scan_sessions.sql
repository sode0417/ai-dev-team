-- scan_sessions テーブル
CREATE TABLE scan_sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    status TEXT NOT NULL DEFAULT 'running',
    analysis TEXT,
    priority_actions JSONB,
    retrospective TEXT,
    improvement_suggestions JSONB,
    error_log TEXT,
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ
);

CREATE INDEX idx_scan_sessions_project_id ON scan_sessions(project_id);

-- tasks テーブルに scan_id と proposal_type を追加
ALTER TABLE tasks ADD COLUMN scan_id UUID REFERENCES scan_sessions(id) ON DELETE SET NULL;
ALTER TABLE tasks ADD COLUMN proposal_type TEXT NOT NULL DEFAULT 'development';
