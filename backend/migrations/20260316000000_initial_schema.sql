CREATE TABLE projects (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    description TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE project_repositories (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    owner TEXT NOT NULL,
    name TEXT NOT NULL,
    default_branch TEXT NOT NULL DEFAULT 'main',
    local_path TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(project_id, owner, name)
);

CREATE TYPE task_status AS ENUM (
    'proposed', 'approved', 'queued', 'planning',
    'executing', 'reviewing', 'completed', 'failed', 'cancelled', 'blocked'
);
CREATE TYPE task_priority AS ENUM ('critical', 'high', 'medium', 'low');

CREATE TABLE tasks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects(id),
    repository_id UUID REFERENCES project_repositories(id),
    title TEXT NOT NULL,
    description TEXT NOT NULL,
    status task_status NOT NULL DEFAULT 'proposed',
    priority task_priority NOT NULL DEFAULT 'medium',
    depends_on UUID REFERENCES tasks(id),
    execution_order INT NOT NULL DEFAULT 0,
    proposed_by TEXT NOT NULL DEFAULT 'user',
    plan TEXT,
    pr_url TEXT,
    changed_files JSONB,
    diff_stats TEXT,
    retry_count INT NOT NULL DEFAULT 0,
    max_retries INT NOT NULL DEFAULT 2,
    error_log TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE execution_sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    task_id UUID NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    attempt INT NOT NULL DEFAULT 1,
    status TEXT NOT NULL DEFAULT 'running',
    worktree_path TEXT,
    branch_name TEXT,
    plan_output TEXT,
    review_output TEXT,
    review_verdict TEXT,
    test_output TEXT,
    test_passed BOOLEAN,
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ
);

CREATE TABLE execution_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id UUID NOT NULL REFERENCES execution_sessions(id) ON DELETE CASCADE,
    phase TEXT NOT NULL,
    iteration INT NOT NULL DEFAULT 1,
    level TEXT NOT NULL DEFAULT 'info',
    message TEXT NOT NULL,
    metadata JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_tasks_project_id ON tasks(project_id);
CREATE INDEX idx_tasks_status ON tasks(status);
CREATE INDEX idx_execution_sessions_task_id ON execution_sessions(task_id);
CREATE INDEX idx_execution_logs_session_id ON execution_logs(session_id);
