"use client";

import { usePathname } from "next/navigation";
import { useAuth } from "@/components/AuthProvider";
import { Sidebar } from "@/components/Sidebar";

export function AppShell({ children }: { children: React.ReactNode }) {
  const { user, loading } = useAuth();
  const pathname = usePathname();

  // ログインページは認証不要でそのまま表示
  if (pathname === "/login") {
    return <>{children}</>;
  }

  // ローディング中はスピナー、未認証時は null（フラッシュ防止）
  if (loading) {
    return (
      <div className="min-h-screen flex items-center justify-center">
        <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-gray-900" />
      </div>
    );
  }

  if (!user) {
    return null;
  }

  return (
    <div className="min-h-screen flex">
      <Sidebar />
      <main className="flex-1 p-4 pt-16 lg:p-6 lg:pt-6 overflow-auto">
        {children}
      </main>
    </div>
  );
}
