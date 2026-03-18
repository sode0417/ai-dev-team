function getApiBase(): string {
  if (process.env.NEXT_PUBLIC_API_URL) return process.env.NEXT_PUBLIC_API_URL;
  if (typeof window !== "undefined" && window.location.hostname !== "localhost") {
    // devteam.sode-ai.com → devteam-api.sode-ai.com
    const apiHost = window.location.hostname.replace(
      /^([^.]+)\./,
      "$1-api."
    );
    return `${window.location.protocol}//${apiHost}`;
  }
  return "http://localhost:8100";
}

export const API_BASE = getApiBase();
