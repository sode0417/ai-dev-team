"use client";

import {
  createContext,
  useContext,
  useEffect,
  useState,
  useCallback,
  type ReactNode,
} from "react";
import type { User } from "@/types";
import {
  getAccessToken,
  clearTokens,
  login as authLogin,
  refreshAccessToken,
} from "@/lib/auth";
import { API_BASE } from "@/lib/config";

interface AuthContextValue {
  user: User | null;
  loading: boolean;
  login: (username: string, password: string) => Promise<void>;
  logout: () => void;
}

const AuthContext = createContext<AuthContextValue>({
  user: null,
  loading: true,
  login: async () => {},
  logout: () => {},
});

export function useAuth() {
  return useContext(AuthContext);
}

export function AuthProvider({ children }: { children: ReactNode }) {
  const [user, setUser] = useState<User | null>(null);
  const [loading, setLoading] = useState(true);

  const fetchMe = useCallback(async (): Promise<User | null> => {
    const token = getAccessToken();
    if (!token) return null;

    try {
      const res = await fetch(`${API_BASE}/api/auth/me`, {
        headers: { Authorization: `Bearer ${token}` },
      });

      if (res.ok) {
        const { data } = (await res.json()) as { data: User };
        return data;
      }

      if (res.status === 401) {
        const refreshed = await refreshAccessToken();
        if (refreshed) {
          const newToken = getAccessToken();
          if (newToken) {
            const retryRes = await fetch(`${API_BASE}/api/auth/me`, {
              headers: { Authorization: `Bearer ${newToken}` },
            });
            if (retryRes.ok) {
              const { data } = (await retryRes.json()) as { data: User };
              return data;
            }
          }
        }
        clearTokens();
      }
    } catch {
      // ネットワークエラー等
    }
    return null;
  }, []);

  useEffect(() => {
    fetchMe().then((u) => {
      setUser(u);
      setLoading(false);
    });
  }, [fetchMe]);

  // 未認証時のリダイレクト（window.location を使用して SSR 安全に）
  useEffect(() => {
    if (!loading && !user && typeof window !== "undefined") {
      const path = window.location.pathname;
      if (path !== "/login") {
        window.location.href = "/login";
      }
    }
  }, [loading, user]);

  const login = useCallback(
    async (username: string, password: string) => {
      await authLogin(username, password);
      const u = await fetchMe();
      setUser(u);
      window.location.href = "/";
    },
    [fetchMe]
  );

  const logout = useCallback(() => {
    clearTokens();
    setUser(null);
    window.location.href = "/login";
  }, []);

  return (
    <AuthContext.Provider value={{ user, loading, login, logout }}>
      {children}
    </AuthContext.Provider>
  );
}
