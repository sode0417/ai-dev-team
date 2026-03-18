-- Issue 単独実行機能: tasks テーブルに Issue 関連カラムを追加
ALTER TABLE tasks ADD COLUMN issue_number INT;
ALTER TABLE tasks ADD COLUMN issue_url TEXT;
