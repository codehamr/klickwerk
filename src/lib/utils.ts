import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}
export function errorText(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}
export function formatTime(ms: number) {
  const seconds = Math.floor(ms / 1000);
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
}

export function normalizeServerUrl(value: string): URL {
  const text = value.trim();
  const local =
    /^(localhost|127\.|192\.168\.|10\.|\[::1\]|\d{1,3}(?:\.\d{1,3}){3}(?::|\/|$))/.test(
      text,
    );
  const url = new URL(
    text.includes("://") ? text : `${local ? "http" : "https"}://${text}`,
  );
  if (
    !["http:", "https:"].includes(url.protocol) ||
    url.username ||
    url.password ||
    url.search ||
    url.hash
  )
    throw new Error(
      "Use an HTTP(S) URL without credentials, query parameters, or fragments.",
    );
  url.pathname = `${url.pathname.replace(/\/$/, "").replace(/\/chat\/completions$/, "") || "/v1"}/`;
  return url;
}
