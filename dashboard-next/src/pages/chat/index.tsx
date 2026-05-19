import { createResource, createSignal, For, Show, type Component } from "solid-js";
import { useParams } from "@solidjs/router";
import { apiGet, apiPost } from "@/api/client";
import { Button, Textarea } from "@/components/Form";
import { Card, EmptyState, Loading, PageHeader } from "@/components/Card";
import { MessagePartsRenderer, type MessagePart } from "@/components/MessagePartsRenderer";
import { toastError } from "@/components/Toast";

interface ChatMessage {
  id?: string;
  role?: string;
  content?: string;
  parts?: MessagePart[];
  ts?: string;
  streaming?: boolean;
}

interface ConversationDetail {
  id?: string;
  title?: string;
  messages?: ChatMessage[];
}

const ChatPage: Component = () => {
  const params = useParams();
  const [conversation, { refetch }] = createResource<ConversationDetail>(async () => {
    const id = params.id;
    if (id) {
      return apiGet<ConversationDetail>(`/api/management/chat/conversation/${id}`).catch(() => ({}));
    }
    return apiGet<ConversationDetail>("/api/management/chat/new").catch(() => ({}));
  });

  const [input, setInput] = createSignal("");
  const [pending, setPending] = createSignal(false);

  const send = async () => {
    const text = input().trim();
    if (!text) return;
    setPending(true);
    try {
      await apiPost("/api/management/chat/send", {
        conversation_id: params.id ?? conversation()?.id ?? null,
        content: text,
      });
      setInput("");
      await refetch();
    } catch (err) {
      toastError(err);
    } finally {
      setPending(false);
    }
  };

  return (
    <>
      <PageHeader title="对话" subtitle={conversation()?.title ?? "Web 调试对话"} />
      <Card>
        <Show when={!conversation.loading} fallback={<Loading />}>
          <Show when={conversation()?.messages?.length} fallback={<EmptyState message="尚无消息" />}>
            <div style={{ display: "flex", "flex-direction": "column", gap: "8px" }}>
              <For each={conversation()?.messages ?? []}>
                {(m) => (
                  <div class="card" style={{ background: m.role === "user" ? "var(--bg-2)" : "var(--bg-1)" }}>
                    <div class="muted text-mono" style={{ "font-size": "11px" }}>
                      {m.role ?? "?"} · {m.ts ?? ""}
                    </div>
                    <MessagePartsRenderer
                      parts={m.parts}
                      fallbackText={m.content}
                      streaming={m.streaming}
                    />
                  </div>
                )}
              </For>
            </div>
          </Show>
        </Show>
      </Card>
      <Card title="输入">
        <Textarea
          rows={4}
          value={input()}
          onInput={(e) => setInput(e.currentTarget.value)}
          placeholder="输入消息…"
        />
        <div class="row" style={{ "margin-top": "8px" }}>
          <Button variant="primary" disabled={pending()} onClick={send}>
            {pending() ? "发送中…" : "发送"}
          </Button>
        </div>
      </Card>
    </>
  );
};

export default ChatPage;
