-- タスクにキャンセル理由カラムを追加
ALTER TABLE tasks ADD COLUMN cancel_reason TEXT;
