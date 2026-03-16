import type { Metadata } from "next";
import { Geist, Geist_Mono } from "next/font/google";
import Link from "next/link";
import "./globals.css";

const geistSans = Geist({
  variable: "--font-geist-sans",
  subsets: ["latin"],
});

const geistMono = Geist_Mono({
  variable: "--font-geist-mono",
  subsets: ["latin"],
});

export const metadata: Metadata = {
  title: "AI Dev Team",
  description: "PM Agent 主導の自律型開発チーム管理システム",
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="ja">
      <body
        className={`${geistSans.variable} ${geistMono.variable} antialiased`}
      >
        <div className="min-h-screen flex">
          <nav className="w-56 bg-slate-900 text-slate-200 p-4 flex flex-col gap-2">
            <h1 className="text-lg font-bold mb-4 text-white">AI Dev Team</h1>
            <Link
              href="/"
              className="px-3 py-2 rounded hover:bg-slate-800 transition"
            >
              Dashboard
            </Link>
            <Link
              href="/tasks"
              className="px-3 py-2 rounded hover:bg-slate-800 transition"
            >
              Tasks
            </Link>
          </nav>
          <main className="flex-1 p-6">{children}</main>
        </div>
      </body>
    </html>
  );
}
