const TOKEN_STORAGE_KEY = "astrbot.dashboard.token";
const API_KEY_STORAGE_KEY = "astrbot.dashboard.openapi-key";

export interface ApiError extends Error {
  status: number;
  body: unknown;
}

export function getToken(): string | null {
  try {
    return localStorage.getItem(TOKEN_STORAGE_KEY);
  } catch {
    return null;
  }
}

export function setToken(token: string | null): void {
  try {
    if (token === null) {
      localStorage.removeItem(TOKEN_STORAGE_KEY);
    } else {
      localStorage.setItem(TOKEN_STORAGE_KEY, token);
    }
  } catch {
    /* ignore storage errors */
  }
}

export function getApiKey(): string | null {
  try {
    return localStorage.getItem(API_KEY_STORAGE_KEY);
  } catch {
    return null;
  }
}

export function setApiKey(value: string | null): void {
  try {
    if (value === null) {
      localStorage.removeItem(API_KEY_STORAGE_KEY);
    } else {
      localStorage.setItem(API_KEY_STORAGE_KEY, value);
    }
  } catch {
    /* ignore */
  }
}

function buildHeaders(init?: HeadersInit, extra?: HeadersInit): Headers {
  const headers = new Headers(init);
  if (extra) {
    new Headers(extra).forEach((value, key) => headers.set(key, value));
  }
  const token = getToken();
  if (token && !headers.has("Authorization")) {
    headers.set("Authorization", `Bearer ${token}`);
  }
  return headers;
}

async function parseError(res: Response): Promise<ApiError> {
  let body: unknown = null;
  const text = await res.text().catch(() => "");
  if (text) {
    try {
      body = JSON.parse(text);
    } catch {
      body = text;
    }
  }
  const message =
    (body && typeof body === "object" && "error" in body && typeof (body as { error: unknown }).error === "string"
      ? (body as { error: string }).error
      : null) ?? `HTTP ${res.status} ${res.statusText}`;
  const error = new Error(message) as ApiError;
  error.status = res.status;
  error.body = body;
  return error;
}

export async function apiFetch(path: string, init: RequestInit = {}): Promise<Response> {
  const headers = buildHeaders(init.headers);
  const res = await fetch(path, { ...init, headers });
  if (!res.ok) {
    throw await parseError(res);
  }
  return res;
}

export async function apiGet<T>(path: string): Promise<T> {
  const res = await apiFetch(path, { method: "GET" });
  return (await res.json()) as T;
}

export async function apiPost<T, B = unknown>(path: string, body?: B): Promise<T> {
  const init: RequestInit = {
    method: "POST",
    headers: { "Content-Type": "application/json" },
  };
  if (body !== undefined) {
    init.body = JSON.stringify(body);
  }
  const res = await apiFetch(path, init);
  const text = await res.text();
  if (!text) return undefined as T;
  return JSON.parse(text) as T;
}

export async function apiPostMultipart<T>(path: string, form: FormData): Promise<T> {
  const res = await apiFetch(path, { method: "POST", body: form });
  return (await res.json()) as T;
}

export function buildEventSource(path: string): EventSource {
  const url = new URL(path, window.location.origin);
  const token = getToken();
  if (token) {
    url.searchParams.set("token", token);
  }
  return new EventSource(url.toString(), { withCredentials: false });
}
