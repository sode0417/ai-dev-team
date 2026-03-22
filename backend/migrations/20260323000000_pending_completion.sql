-- 新ステータス追加
ALTER TYPE task_status ADD VALUE 'pending_completion' BEFORE 'completed';

-- 完了確認メモカラム追加
ALTER TABLE tasks ADD COLUMN completion_note TEXT;
