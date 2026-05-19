import {
  For,
  Show,
  createMemo,
  type Component,
  type JSX,
} from "solid-js";
import { Input } from "./Form";

/**
 * JSON Schema subset understood by SchemaForm. We deliberately keep the
 * vocabulary small so the component stays a single file:
 *
 * - `type`: "string" | "number" | "integer" | "boolean" | "object" | "array"
 * - `title`: human label
 * - `description`: hint text below the field
 * - `default`: prefilled value
 * - `enum`: render as <select>
 * - `properties`: child fields when type is "object"
 * - `required`: marks fields with `*`
 * - `items`: schema for array elements
 * - `format`: "password" | "uri" | "textarea"
 *
 * Anything else is ignored.
 */
export interface JsonSchema {
  type?: "string" | "number" | "integer" | "boolean" | "object" | "array";
  title?: string;
  description?: string;
  default?: unknown;
  enum?: unknown[];
  properties?: Record<string, JsonSchema>;
  required?: string[];
  items?: JsonSchema;
  format?: "password" | "uri" | "textarea";
  minimum?: number;
  maximum?: number;
  minLength?: number;
  maxLength?: number;
}

export interface SchemaFormProps {
  schema: JsonSchema;
  value: unknown;
  onChange: (next: unknown) => void;
  disabled?: boolean | undefined;
  /** Optional path label override; lookups use dot-separated key path. */
  labels?: Record<string, string>;
}

interface FieldProps {
  schema: JsonSchema;
  value: unknown;
  onChange: (next: unknown) => void;
  path: string;
  required?: boolean | undefined;
  disabled?: boolean | undefined;
  labels?: Record<string, string> | undefined;
}

const ObjectField: Component<FieldProps> = (props): JSX.Element => {
  const properties = createMemo(() => props.schema.properties ?? {});
  const required = createMemo(() => new Set(props.schema.required ?? []));
  const currentObject = createMemo(() => {
    const value = props.value;
    if (value && typeof value === "object" && !Array.isArray(value)) {
      return value as Record<string, unknown>;
    }
    return {} as Record<string, unknown>;
  });

  const updateChild = (key: string, next: unknown) => {
    const merged = { ...currentObject(), [key]: next };
    props.onChange(merged);
  };

  return (
    <div class="schema-form__object">
      <For each={Object.keys(properties())}>
        {(key) => {
          const childSchema = properties()[key]!;
          const childPath = props.path ? `${props.path}.${key}` : key;
          const labelText = createMemo(
            () =>
              props.labels?.[childPath] ??
              childSchema.title ??
              prettifyKey(key)
          );
          return (
            <label class="field">
              <span class="field__label">
                {labelText()}
                <Show when={required().has(key)}>
                  <span style={{ color: "var(--color-danger)" }}> *</span>
                </Show>
              </span>
              <SchemaField
                schema={childSchema}
                value={currentObject()[key]}
                onChange={(next) => updateChild(key, next)}
                path={childPath}
                required={required().has(key)}
                disabled={props.disabled}
                labels={props.labels}
              />
              <Show when={childSchema.description}>
                <span class="field__hint">{childSchema.description}</span>
              </Show>
            </label>
          );
        }}
      </For>
    </div>
  );
};

const ArrayField: Component<FieldProps> = (props) => {
  const list = createMemo(() => {
    const value = props.value;
    return Array.isArray(value) ? (value as unknown[]) : [];
  });
  const itemSchema: JsonSchema = props.schema.items ?? { type: "string" };
  const addItem = () => {
    const next = [...list(), defaultForSchema(itemSchema)];
    props.onChange(next);
  };
  const removeItem = (index: number) => {
    const next = list().filter((_, i) => i !== index);
    props.onChange(next);
  };
  const updateItem = (index: number, value: unknown) => {
    const next = [...list()];
    next[index] = value;
    props.onChange(next);
  };

  return (
    <div class="schema-form__array">
      <For each={list()}>
        {(item, idx) => (
          <div class="schema-form__array-row">
            <SchemaField
              schema={itemSchema}
              value={item}
              onChange={(next) => updateItem(idx(), next)}
              path={`${props.path}[${idx()}]`}
              disabled={props.disabled}
              labels={props.labels}
            />
            <button
              type="button"
              class="btn btn--ghost btn--sm"
              disabled={props.disabled}
              onClick={() => removeItem(idx())}
            >
              删除
            </button>
          </div>
        )}
      </For>
      <button
        type="button"
        class="btn btn--ghost btn--sm"
        disabled={props.disabled}
        onClick={addItem}
      >
        + 添加
      </button>
    </div>
  );
};

const PrimitiveField: Component<FieldProps> = (props): JSX.Element => {
  const enumValues = props.schema.enum;
  if (enumValues && enumValues.length > 0) {
    return (
      <select
        class="input"
        disabled={props.disabled}
        value={String(props.value ?? "")}
        onChange={(event) => {
          const raw = event.currentTarget.value;
          const matched = enumValues.find((candidate) => String(candidate) === raw);
          props.onChange(matched ?? raw);
        }}
      >
        <For each={enumValues}>
          {(opt) => <option value={String(opt)}>{String(opt)}</option>}
        </For>
      </select>
    );
  }
  if (props.schema.type === "boolean") {
    return (
      <label class="row">
        <input
          type="checkbox"
          checked={Boolean(props.value)}
          disabled={props.disabled}
          onChange={(event) => props.onChange(event.currentTarget.checked)}
        />
        <span class="muted">
          {Boolean(props.value) ? "开启" : "关闭"}
        </span>
      </label>
    );
  }
  if (props.schema.type === "number" || props.schema.type === "integer") {
    const step = props.schema.type === "integer" ? 1 : "any";
    return (
      <Input
        type="number"
        value={(props.value as number | undefined) ?? ""}
        step={step}
        min={props.schema.minimum}
        max={props.schema.maximum}
        disabled={props.disabled}
        onInput={(event) => {
          const text = event.currentTarget.value;
          if (text === "") {
            props.onChange(undefined);
            return;
          }
          const parsed = Number(text);
          if (Number.isFinite(parsed)) {
            props.onChange(
              props.schema.type === "integer" ? Math.trunc(parsed) : parsed
            );
          }
        }}
      />
    );
  }
  if (props.schema.format === "textarea") {
    return (
      <textarea
        class="input"
        rows={4}
        value={(props.value as string | undefined) ?? ""}
        disabled={props.disabled}
        onInput={(event) => props.onChange(event.currentTarget.value)}
      />
    );
  }
  const inputType =
    props.schema.format === "password"
      ? "password"
      : props.schema.format === "uri"
        ? "url"
        : "text";
  return (
    <Input
      type={inputType}
      value={(props.value as string | undefined) ?? ""}
      minLength={props.schema.minLength}
      maxLength={props.schema.maxLength}
      disabled={props.disabled}
      onInput={(event) => props.onChange(event.currentTarget.value)}
    />
  );
};

const SchemaField: Component<FieldProps> = (props): JSX.Element => {
  if (props.schema.type === "object") {
    return <ObjectField {...props} />;
  }
  if (props.schema.type === "array") {
    return <ArrayField {...props} />;
  }
  return <PrimitiveField {...props} />;
};

/**
 * Top-level renderer. Use this directly in pages; pass the schema returned
 * by the management API and bind value/onChange into a Solid store.
 */
export const SchemaForm: Component<SchemaFormProps> = (props): JSX.Element => {
  return (
    <SchemaField
      schema={props.schema}
      value={props.value}
      onChange={props.onChange}
      path=""
      disabled={props.disabled}
      labels={props.labels}
    />
  );
};

export function defaultForSchema(schema: JsonSchema): unknown {
  if (schema.default !== undefined) return schema.default;
  switch (schema.type) {
    case "string":
      return "";
    case "number":
    case "integer":
      return 0;
    case "boolean":
      return false;
    case "array":
      return [];
    case "object": {
      const out: Record<string, unknown> = {};
      for (const [key, child] of Object.entries(schema.properties ?? {})) {
        out[key] = defaultForSchema(child);
      }
      return out;
    }
    default:
      return null;
  }
}

/**
 * Validate a value against a schema. Returns an array of `{ path, message }`
 * for each violation. Empty array means valid.
 */
export interface SchemaValidationIssue {
  path: string;
  message: string;
}

export function validateAgainstSchema(
  schema: JsonSchema,
  value: unknown,
  path: string = ""
): SchemaValidationIssue[] {
  const out: SchemaValidationIssue[] = [];
  if (schema.type === "object") {
    if (
      value === null ||
      value === undefined ||
      typeof value !== "object" ||
      Array.isArray(value)
    ) {
      out.push({ path, message: "expected object" });
      return out;
    }
    const obj = value as Record<string, unknown>;
    for (const key of schema.required ?? []) {
      if (obj[key] === undefined || obj[key] === null || obj[key] === "") {
        out.push({
          path: path ? `${path}.${key}` : key,
          message: "required",
        });
      }
    }
    for (const [key, child] of Object.entries(schema.properties ?? {})) {
      if (obj[key] === undefined) continue;
      out.push(
        ...validateAgainstSchema(
          child,
          obj[key],
          path ? `${path}.${key}` : key
        )
      );
    }
    return out;
  }
  if (schema.type === "array") {
    if (!Array.isArray(value)) {
      if (value !== undefined) out.push({ path, message: "expected array" });
      return out;
    }
    const itemSchema = schema.items;
    if (itemSchema) {
      value.forEach((item, idx) => {
        out.push(
          ...validateAgainstSchema(itemSchema, item, `${path}[${idx}]`)
        );
      });
    }
    return out;
  }
  if (
    schema.enum &&
    schema.enum.length > 0 &&
    value !== undefined &&
    !schema.enum.some((candidate) => candidate === value)
  ) {
    out.push({
      path,
      message: `value must be one of ${schema.enum.map(String).join(", ")}`,
    });
  }
  if (schema.type === "string" && value !== undefined && typeof value !== "string") {
    out.push({ path, message: "expected string" });
  }
  if (
    (schema.type === "number" || schema.type === "integer") &&
    value !== undefined &&
    (typeof value !== "number" || Number.isNaN(value))
  ) {
    out.push({ path, message: "expected number" });
  }
  if (
    schema.type === "boolean" &&
    value !== undefined &&
    typeof value !== "boolean"
  ) {
    out.push({ path, message: "expected boolean" });
  }
  return out;
}

function prettifyKey(key: string): string {
  return key
    .replace(/[_-]+/g, " ")
    .replace(/([a-z])([A-Z])/g, "$1 $2")
    .replace(/^./, (c) => c.toUpperCase());
}

export default SchemaForm;
