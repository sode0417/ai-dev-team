-- タスク並列実行: execution_group による並列/直列制御
ALTER TABLE tasks ADD COLUMN execution_group INT NOT NULL DEFAULT 0;
ALTER TABLE sprints ADD COLUMN max_parallel_tasks INT NOT NULL DEFAULT 3;
