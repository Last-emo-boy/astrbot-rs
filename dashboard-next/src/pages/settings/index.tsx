import { createSignal, type Component } from "solid-js";
import { Button, Field, Input } from "@/components/Form";
import { Card, PageHeader } from "@/components/Card";
import { getApiKey, setApiKey, setToken, getToken } from "@/api/client";
import { toastSuccess } from "@/components/Toast";

const SettingsPage: Component = () => {
  const [token, setTokenInput] = createSignal(getToken() ?? "");
  const [apiKey, setApiKeyInput] = createSignal(getApiKey() ?? "");

  return (
    <>
      <PageHeader title="设置" subtitle="管理 Dashboard token 与 OpenAPI 调试密钥" />
      <Card title="Dashboard Token">
        <Field label="Bearer Token" hint="用于管理 API 鉴权，登录后自动写入。可在此处手动覆盖。">
          <Input value={token()} onInput={(e) => setTokenInput(e.currentTarget.value)} />
        </Field>
        <Button
          variant="primary"
          onClick={() => {
            setToken(token() || null);
            toastSuccess("Token saved");
          }}
        >
          保存
        </Button>
      </Card>
      <Card title="OpenAPI 调试密钥">
        <Field label="Chat API Key" hint="用于在 /chat 页面以 OpenAPI 模式调用对话接口（chat scope）">
          <Input value={apiKey()} onInput={(e) => setApiKeyInput(e.currentTarget.value)} />
        </Field>
        <Button
          variant="primary"
          onClick={() => {
            setApiKey(apiKey() || null);
            toastSuccess("API key saved");
          }}
        >
          保存
        </Button>
      </Card>
    </>
  );
};

export default SettingsPage;
