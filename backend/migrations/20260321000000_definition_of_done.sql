-- タスクに完了条件 (Definition of Done) カラムを追加
ALTER TABLE tasks ADD COLUMN definition_of_done TEXT;
