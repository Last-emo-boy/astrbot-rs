import { createSignal } from "solid-js";
import { apiPost, getToken, setToken } from "./client";

export interface LoginRequest {
  username: string;
  password: string;
}

export interface AuthResponse {
  token?: string | null;
  username?: string | null;
  expires_at?: string | null;
  error?: string | null;
}

const [token, setTokenSignal] = createSignal<string | null>(getToken());
const [username, setUsername] = createSignal<string | null>(null);

export { token, username };

export async function login(payload: LoginRequest): Promise<AuthResponse> {
  const res = await apiPost<AuthResponse, LoginRequest>("/api/auth/login", payload);
  if (res.token) {
    setToken(res.token);
    setTokenSignal(res.token);
  }
  if (res.username) {
    setUsername(res.username);
  }
  return res;
}

export function logout(): void {
  setToken(null);
  setTokenSignal(null);
  setUsername(null);
}

export function isAuthenticated(): boolean {
  return token() !== null;
}
