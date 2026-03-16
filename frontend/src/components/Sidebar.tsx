"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { usePathname } from "next/navigation";
import { fetchProjects } from "@/lib/api";
import type { Project } from "@/types";

export function Sidebar() {
  const pathname = usePathname();
  const [projects, setProjects] = useState<Project[]>([]);
  const [open, setOpen] = useState(false);

  useEffect(() => {
    fetchProjects()
      .then((res) => setProjects(res.data))
      .catch(() => {});
  }, []);

  // ページ遷移時にモバイルメニューを閉じる
  useEffect(() => {
    setOpen(false);
  }, [pathname]);

  const linkClass = (href: string) => {
    const active = pathname === href;
    return `flex items-center gap-2.5 px-3 py-2 rounded-md transition text-sm ${
      active
        ? "bg-gh-surface text-gh-text font-medium"
        : "text-gh-text-secondary hover:bg-gh-surface hover:text-gh-text"
    }`;
  };

  const navContent = (
    <>
      <div className="p-4 pb-2 flex items-center justify-between">
        <Link href="/" className="flex items-center gap-2 group">
          <div className="w-7 h-7 rounded-md bg-gh-purple/20 flex items-center justify-center text-gh-purple text-xs font-bold">
            AI
          </div>
          <span className="text-sm font-semibold text-gh-text group-hover:text-gh-link transition">
            AI Dev Team
          </span>
        </Link>
        {/* モバイル閉じるボタン */}
        <button
          onClick={() => setOpen(false)}
          className="lg:hidden p-1 text-gh-text-secondary hover:text-gh-text"
        >
          <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
            <path strokeLinecap="round" strokeLinejoin="round" d="M6 18 18 6M6 6l12 12" />
          </svg>
        </button>
      </div>

      <div className="flex-1 px-3 py-2 space-y-0.5">
        <Link href="/" className={linkClass("/")}>
          <svg className="w-4 h-4 opacity-70" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
            <path strokeLinecap="round" strokeLinejoin="round" d="M3.75 6A2.25 2.25 0 0 1 6 3.75h2.25A2.25 2.25 0 0 1 10.5 6v2.25a2.25 2.25 0 0 1-2.25 2.25H6a2.25 2.25 0 0 1-2.25-2.25V6ZM3.75 15.75A2.25 2.25 0 0 1 6 13.5h2.25a2.25 2.25 0 0 1 2.25 2.25V18a2.25 2.25 0 0 1-2.25 2.25H6A2.25 2.25 0 0 1 3.75 18v-2.25ZM13.5 6a2.25 2.25 0 0 1 2.25-2.25H18A2.25 2.25 0 0 1 20.25 6v2.25A2.25 2.25 0 0 1 18 10.5h-2.25a2.25 2.25 0 0 1-2.25-2.25V6ZM13.5 15.75a2.25 2.25 0 0 1 2.25-2.25H18a2.25 2.25 0 0 1 2.25 2.25V18A2.25 2.25 0 0 1 18 20.25h-2.25a2.25 2.25 0 0 1-2.25-2.25v-2.25Z" />
          </svg>
          Dashboard
        </Link>
        {projects.length > 0 && (
          <div className="pt-4">
            <div className="text-[11px] text-gh-text-muted uppercase tracking-wider px-3 mb-1.5">
              Projects
            </div>
            {projects.map((p) => (
              <Link
                key={p.id}
                href={`/projects/${p.id}`}
                className={linkClass(`/projects/${p.id}`)}
                title={p.name}
              >
                <svg className="w-4 h-4 opacity-70 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
                  <path strokeLinecap="round" strokeLinejoin="round" d="M2.25 12.75V12A2.25 2.25 0 0 1 4.5 9.75h15A2.25 2.25 0 0 1 21.75 12v.75m-8.69-6.44-2.12-2.12a1.5 1.5 0 0 0-1.061-.44H4.5A2.25 2.25 0 0 0 2.25 6v12a2.25 2.25 0 0 0 2.25 2.25h15A2.25 2.25 0 0 0 21.75 18V9a2.25 2.25 0 0 0-2.25-2.25h-5.379a1.5 1.5 0 0 1-1.06-.44Z" />
                </svg>
                <span className="truncate">{p.name}</span>
              </Link>
            ))}
          </div>
        )}
      </div>
    </>
  );

  return (
    <>
      {/* モバイルヘッダー */}
      <div className="lg:hidden fixed top-0 left-0 right-0 z-40 bg-gh-sidebar border-b border-gh-border px-4 py-3 flex items-center gap-3">
        <button
          onClick={() => setOpen(true)}
          className="p-1 text-gh-text-secondary hover:text-gh-text"
        >
          <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
            <path strokeLinecap="round" strokeLinejoin="round" d="M3.75 6.75h16.5M3.75 12h16.5m-16.5 5.25h16.5" />
          </svg>
        </button>
        <Link href="/" className="flex items-center gap-2">
          <div className="w-6 h-6 rounded-md bg-gh-purple/20 flex items-center justify-center text-gh-purple text-[10px] font-bold">
            AI
          </div>
          <span className="text-sm font-semibold text-gh-text">AI Dev Team</span>
        </Link>
      </div>

      {/* モバイルオーバーレイ */}
      {open && (
        <div
          className="lg:hidden fixed inset-0 z-50 bg-black/50"
          onClick={() => setOpen(false)}
        />
      )}

      {/* デスクトップ: 常時表示 / モバイル: スライドイン */}
      <nav
        className={`
          fixed lg:sticky top-0 left-0 z-50 h-screen
          w-56 bg-gh-sidebar border-r border-gh-border flex flex-col shrink-0
          transition-transform duration-200
          ${open ? "translate-x-0" : "-translate-x-full lg:translate-x-0"}
        `}
      >
        {navContent}
      </nav>
    </>
  );
}
