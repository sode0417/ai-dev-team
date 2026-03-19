-- PR作成後の修正フロー対応

-- タスクに修正回数を追加（振り返りの材料として追跡）
ALTER TABLE tasks ADD COLUMN revision_count INT NOT NULL DEFAULT 0;

-- execution_sessions に修正依頼内容を追加（修正セッションの記録）
ALTER TABLE execution_sessions ADD COLUMN revision_instructions TEXT;
