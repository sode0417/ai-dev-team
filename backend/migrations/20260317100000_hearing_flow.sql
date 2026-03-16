-- ヒアリング・計画承認フロー用マイグレーション

-- task_status に hearing と awaiting_approval を追加
ALTER TYPE task_status ADD VALUE IF NOT EXISTS 'hearing' BEFORE 'planning';
ALTER TYPE task_status ADD VALUE IF NOT EXISTS 'awaiting_approval' BEFORE 'executing';

-- ヒアリング記録テーブル
CREATE TABLE IF NOT EXISTS task_hearings (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    task_id UUID NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    session_id UUID REFERENCES execution_sessions(id) ON DELETE CASCADE,
    phase TEXT NOT NULL DEFAULT 'pre_plan',   -- 'pre_plan' | 'in_plan'
    round INT NOT NULL DEFAULT 1,
    questions JSONB NOT NULL,                  -- [{index, question, options?}]
    answers JSONB,                             -- [{index, answer}]
    status TEXT NOT NULL DEFAULT 'pending',    -- 'pending' | 'answered' | 'skipped'
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    answered_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_task_hearings_task_id ON task_hearings(task_id);
