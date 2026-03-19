-- 自動マージ機能: tasks テーブルにマージ状態カラム追加
ALTER TABLE tasks ADD COLUMN merge_status VARCHAR(20) DEFAULT 'pending';
-- pending | merged | conflict | failed

ALTER TABLE tasks ADD COLUMN merge_attempted_at TIMESTAMPTZ;

-- マージ試行ログ
CREATE TABLE merge_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    task_id UUID NOT NULL REFERENCES tasks(id),
    action VARCHAR(20) NOT NULL,  -- 'check' | 'merge' | 'resolve_conflict' | 'notify'
    success BOOLEAN NOT NULL,
    message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_merge_logs_task_id ON merge_logs(task_id);
