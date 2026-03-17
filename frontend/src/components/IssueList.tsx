"use client";

import { useEffect, useState } from "react";
import { fetchRepositoryIssues } from "@/lib/api";
import type { GitHubIssue } from "@/types";

export function IssueList({
  projectId,
  repoId,
}: {
  projectId: string;
  repoId: string;
}) {
  const [issues, setIssues] = useState<GitHubIssue[]>([]);
  const [state, setState] = useState<"open" | "closed">("open");
  const [page, setPage] = useState(1);
  const [loading, setLoading] = useState(true);
  const [hasMore, setHasMore] = useState(false);

  const load = (p: number, s: string, append = false) => {
    setLoading(true);
    fetchRepositoryIssues(projectId, repoId, { state: s, page: p, per_page: 20 })
      .then((res) => {
        setIssues((prev) => (append ? [...prev, ...res.data] : res.data));
        setHasMore(res.data.length === 20);
      })
      .catch(() => setIssues([]))
      .finally(() => setLoading(false));
  };

  useEffect(() => {
    setPage(1);
    load(1, state);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [state, projectId, repoId]);

  const loadMore = () => {
    const next = page + 1;
    setPage(next);
    load(next, state, true);
  };

  return (
    <div className="mt-3">
      <div className="flex gap-1 mb-2">
        <button
          onClick={() => setState("open")}
          className={`text-xs px-2.5 py-1 rounded-md font-medium transition ${
            state === "open"
              ? "bg-gh-green/15 text-gh-green"
              : "text-gh-text-secondary hover:text-gh-text"
          }`}
        >
          Open
        </button>
        <button
          onClick={() => setState("closed")}
          className={`text-xs px-2.5 py-1 rounded-md font-medium transition ${
            state === "closed"
              ? "bg-gh-purple/15 text-gh-purple"
              : "text-gh-text-secondary hover:text-gh-text"
          }`}
        >
          Closed
        </button>
      </div>

      {loading && issues.length === 0 ? (
        <p className="text-gh-text-muted text-xs py-2">読み込み中...</p>
      ) : issues.length === 0 ? (
        <p className="text-gh-text-muted text-xs py-2">Issues はありません</p>
      ) : (
        <div>
          {issues.map((issue, i) => (
            <a
              key={issue.number}
              href={issue.html_url}
              target="_blank"
              rel="noopener noreferrer"
              className={`flex items-start gap-2.5 px-2 py-2.5 hover:bg-gh-overlay rounded-md transition ${
                i > 0 ? "border-t border-gh-border-muted" : ""
              }`}
            >
              {/* Issue icon */}
              <svg className={`w-4 h-4 mt-0.5 shrink-0 ${state === "open" ? "text-gh-green" : "text-gh-purple"}`} viewBox="0 0 16 16" fill="currentColor">
                {state === "open" ? (
                  <path d="M8 9.5a1.5 1.5 0 1 0 0-3 1.5 1.5 0 0 0 0 3Z M8 0a8 8 0 1 1 0 16A8 8 0 0 1 8 0ZM1.5 8a6.5 6.5 0 1 0 13 0 6.5 6.5 0 0 0-13 0Z" />
                ) : (
                  <path d="M11.28 6.78a.75.75 0 0 0-1.06-1.06L7.25 8.69 5.78 7.22a.75.75 0 0 0-1.06 1.06l2 2a.75.75 0 0 0 1.06 0l3.5-3.5ZM16 8A8 8 0 1 1 0 8a8 8 0 0 1 16 0Zm-1.5 0a6.5 6.5 0 1 0-13 0 6.5 6.5 0 0 0 13 0Z" />
                )}
              </svg>
              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-1.5 flex-wrap">
                  <span className="text-sm font-medium text-gh-text hover:text-gh-link">
                    {issue.title}
                  </span>
                  {issue.labels.map((label) => (
                    <span
                      key={label.name}
                      className="text-[10px] px-1.5 py-0.5 rounded-full font-medium border"
                      style={{
                        backgroundColor: `#${label.color}18`,
                        color: `#${label.color}`,
                        borderColor: `#${label.color}30`,
                      }}
                    >
                      {label.name}
                    </span>
                  ))}
                </div>
                <div className="text-[11px] text-gh-text-muted mt-0.5">
                  #{issue.number} · {issue.user?.login} · {new Date(issue.created_at).toLocaleDateString("ja-JP")}
                  {issue.comments > 0 && (
                    <span className="ml-2 inline-flex items-center gap-0.5">
                      <svg className="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                        <path strokeLinecap="round" strokeLinejoin="round" d="M12 20.25c4.97 0 9-3.694 9-8.25s-4.03-8.25-9-8.25S3 7.444 3 12c0 2.104.859 4.023 2.273 5.48.432.447.74 1.04.586 1.641a4.483 4.483 0 0 1-.923 1.785A5.969 5.969 0 0 0 6 21c1.282 0 2.47-.402 3.445-1.087.81.22 1.668.337 2.555.337Z" />
                      </svg>
                      {issue.comments}
                    </span>
                  )}
                </div>
              </div>
            </a>
          ))}
          {hasMore && (
            <button
              onClick={loadMore}
              disabled={loading}
              className="text-xs text-gh-link hover:underline disabled:opacity-50 mt-2 ml-2"
            >
              {loading ? "読み込み中..." : "もっと表示"}
            </button>
          )}
        </div>
      )}
    </div>
  );
}
