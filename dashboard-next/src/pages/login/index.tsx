import { useNavigate } from "@solidjs/router";
import { createSignal, Show, type Component } from "solid-js";
import { login } from "@/api/auth";
import { Button, Field, Input } from "@/components/Form";
import { toastError } from "@/components/Toast";
import { t } from "@/i18n";

const LoginPage: Component = () => {
  const navigate = useNavigate();
  const [username, setUsername] = createSignal("");
  const [password, setPassword] = createSignal("");
  const [pending, setPending] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);

  const submit = async (event: SubmitEvent) => {
    event.preventDefault();
    setError(null);
    setPending(true);
    try {
      const res = await login({ username: username(), password: password() });
      if (res.token) {
        navigate("/");
      } else {
        setError(res.error ?? t("login.error"));
      }
    } catch (err) {
      toastError(err);
      setError(t("login.error"));
    } finally {
      setPending(false);
    }
  };

  return (
    <div class="auth-shell">
      <form class="auth-shell__card" onSubmit={submit}>
        <h1 style={{ "margin-top": 0 }}>{t("login.title")}</h1>
        <Field label={t("login.username")}>
          <Input
            value={username()}
            onInput={(e) => setUsername(e.currentTarget.value)}
            autocomplete="username"
            required
          />
        </Field>
        <Field label={t("login.password")}>
          <Input
            type="password"
            value={password()}
            onInput={(e) => setPassword(e.currentTarget.value)}
            autocomplete="current-password"
            required
          />
        </Field>
        <Show when={error()}>
          <div class="field__error" style={{ "margin-bottom": "12px" }}>{error()}</div>
        </Show>
        <Button type="submit" variant="primary" disabled={pending()} class="btn">
          {pending() ? t("common.loading") : t("login.submit")}
        </Button>
      </form>
    </div>
  );
};

export default LoginPage;
