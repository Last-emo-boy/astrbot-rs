import { createResource, type ResourceFetcher, type ResourceOptions, type Resource } from "solid-js";
import type { Accessor, Setter } from "solid-js";
import { apiGet, apiPost } from "./client";

export interface ResourceCtl<T> {
  data: Resource<T | undefined>;
  refetch: (info?: unknown) => T | Promise<T | undefined> | undefined | null;
  mutate: Setter<T | undefined>;
}

export function useGet<T>(path: Accessor<string | false | null | undefined>): ResourceCtl<T> {
  const fetcher: ResourceFetcher<string, T> = async (p) => apiGet<T>(p);
  const opts: ResourceOptions<T, string> = {};
  const [data, { refetch, mutate }] = createResource<T, string>(
    () => {
      const p = path();
      return p ? p : (false as unknown as string);
    },
    fetcher,
    opts
  );
  return { data, refetch, mutate };
}

export interface PostResourceCtl<T> extends ResourceCtl<T> {}

export function usePost<T, B = unknown>(
  path: Accessor<string | false | null | undefined>,
  body: Accessor<B | undefined>
): PostResourceCtl<T> {
  const [data, { refetch, mutate }] = createResource<T, { path: string; body: B | undefined }>(
    () => {
      const p = path();
      if (!p) return false as unknown as { path: string; body: B | undefined };
      return { path: p, body: body() };
    },
    async ({ path: p, body: b }) => apiPost<T, B | undefined>(p, b)
  );
  return { data, refetch, mutate };
}
