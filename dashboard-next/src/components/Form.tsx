import type { Component, JSX } from "solid-js";
import { splitProps } from "solid-js";

interface ButtonProps extends JSX.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: "primary" | "default" | "danger" | "ghost";
  size?: "sm" | "md";
}

export const Button: Component<ButtonProps> = (rawProps) => {
  const [local, others] = splitProps(rawProps, ["variant", "size", "class"]);
  const cls = () => {
    const base = ["btn"];
    if (local.variant === "primary") base.push("btn--primary");
    if (local.variant === "danger") base.push("btn--danger");
    if (local.variant === "ghost") base.push("btn--ghost");
    if (local.size === "sm") base.push("btn--sm");
    if (local.class) base.push(local.class);
    return base.join(" ");
  };
  return <button type="button" {...others} class={cls()} />;
};

interface InputProps extends JSX.InputHTMLAttributes<HTMLInputElement> {}

export const Input: Component<InputProps> = (rawProps) => {
  const [local, others] = splitProps(rawProps, ["class"]);
  return <input {...others} class={`input ${local.class ?? ""}`} />;
};

interface TextareaProps extends JSX.TextareaHTMLAttributes<HTMLTextAreaElement> {}

export const Textarea: Component<TextareaProps> = (rawProps) => {
  const [local, others] = splitProps(rawProps, ["class"]);
  return <textarea {...others} class={`input ${local.class ?? ""}`} />;
};

interface FieldProps {
  label?: string;
  hint?: string;
  error?: string;
  children: JSX.Element;
}

export const Field: Component<FieldProps> = (props) => (
  <label class="field">
    {props.label && <span class="field__label">{props.label}</span>}
    {props.children}
    {props.hint && <span class="field__hint">{props.hint}</span>}
    {props.error && <span class="field__error">{props.error}</span>}
  </label>
);
