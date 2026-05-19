import { For, Show, createMemo, type Component, type JSX } from "solid-js";
import MarkdownIt from "markdown-it";
import hljs from "highlight.js";
import katex from "katex";

/**
 * One element of a message body. Mirrors astrbot-core::MessageComponent on
 * the wire (snake_case discriminator). New variants must stay
 * backward-compatible — unknown kinds fall through to plain text.
 */
export interface MessagePart {
  kind?: string;
  text?: string;
  url?: string;
  name?: string;
  language?: string;
  /** Reserved for future use. */
  meta?: Record<string, unknown>;
}

interface MessagePartsRendererProps {
  parts?: MessagePart[] | undefined;
  /** Fallback string when `parts` is absent. Rendered as Markdown. */
  fallbackText?: string | undefined;
  /** Show a blinking cursor at the end (streaming response). */
  streaming?: boolean | undefined;
  /** Override classes for the outer container. */
  class?: string | undefined;
}

const md: MarkdownIt = new MarkdownIt({
  html: false,
  linkify: true,
  breaks: true,
  highlight(code: string, lang: string): string {
    if (lang && hljs.getLanguage(lang)) {
      try {
        return `<pre class="hljs"><code>${hljs.highlight(code, { language: lang, ignoreIllegals: true }).value}</code></pre>`;
      } catch {
        /* fall through */
      }
    }
    return `<pre class="hljs"><code>${md.utils.escapeHtml(code)}</code></pre>`;
  },
});

const INLINE_MATH = /\$([^$\n]+?)\$/g;
const BLOCK_MATH = /\$\$([\s\S]+?)\$\$/g;

function renderMath(source: string): string {
  // Block math first so $$...$$ does not get caught by inline first.
  let out = source.replace(BLOCK_MATH, (_match, expr) => {
    try {
      return katex.renderToString(String(expr).trim(), {
        displayMode: true,
        throwOnError: false,
      });
    } catch {
      return _match;
    }
  });
  out = out.replace(INLINE_MATH, (_match, expr) => {
    try {
      return katex.renderToString(String(expr).trim(), {
        displayMode: false,
        throwOnError: false,
      });
    } catch {
      return _match;
    }
  });
  return out;
}

function renderMarkdown(source: string): string {
  // Process $$..$$ / $..$ first so markdown-it doesn't escape backslashes.
  return md.render(renderMath(source));
}

const TextPart: Component<{ text: string }> = (props) => {
  // Plain text — do not Markdown-render. Mirrors the `Plain` enum variant
  // on the Rust side, which is meant to display as-is.
  return <div class="message-part message-part--text">{props.text}</div>;
};

const MarkdownPart: Component<{ text: string }> = (props) => {
  const html = createMemo(() => renderMarkdown(props.text));
  return (
    <div
      class="message-part message-part--markdown"
      innerHTML={html()}
    />
  );
};

const ImagePart: Component<{ url: string; name?: string | undefined }> = (props) => (
  <div class="message-part message-part--image">
    <img src={props.url} alt={props.name ?? ""} style={{ "max-width": "320px", "border-radius": "6px" }} />
    <Show when={props.name}>
      <div class="muted text-mono" style={{ "font-size": "11px", "margin-top": "4px" }}>
        {props.name}
      </div>
    </Show>
  </div>
);

const AudioPart: Component<{ url: string }> = (props) => (
  <div class="message-part message-part--audio">
    {/* eslint-disable-next-line jsx-a11y/media-has-caption */}
    <audio controls src={props.url} style={{ width: "320px" }} />
  </div>
);

const VideoPart: Component<{ url: string }> = (props) => (
  <div class="message-part message-part--video">
    {/* eslint-disable-next-line jsx-a11y/media-has-caption */}
    <video controls src={props.url} style={{ "max-width": "320px", "border-radius": "6px" }} />
  </div>
);

const FilePart: Component<{ url: string; name?: string | undefined }> = (props) => (
  <a class="message-part message-part--file" href={props.url} target="_blank" rel="noreferrer">
    📎 {props.name ?? props.url}
  </a>
);

const CodePart: Component<{ text: string; language?: string | undefined }> = (props) => {
  const html = createMemo(() => {
    const language = props.language ?? "";
    if (language && hljs.getLanguage(language)) {
      try {
        return hljs.highlight(props.text, {
          language,
          ignoreIllegals: true,
        }).value;
      } catch {
        /* fall through */
      }
    }
    return md.utils.escapeHtml(props.text);
  });
  return (
    <pre class="message-part message-part--code hljs">
      <code innerHTML={html()} />
    </pre>
  );
};

const CursorPart: Component = () => (
  <span class="message-part--cursor" aria-hidden="true">
    ▎
  </span>
);

const PartRouter: Component<{ part: MessagePart }> = (props): JSX.Element => {
  const part = props.part;
  switch (part.kind) {
    case "image":
      return part.url ? <ImagePart url={part.url} name={part.name} /> : <></>;
    case "audio":
    case "record":
    case "voice":
      return part.url ? <AudioPart url={part.url} /> : <></>;
    case "video":
      return part.url ? <VideoPart url={part.url} /> : <></>;
    case "file":
      return part.url ? <FilePart url={part.url} name={part.name} /> : <></>;
    case "code":
      return <CodePart text={part.text ?? ""} language={part.language} />;
    case "markdown":
      return <MarkdownPart text={part.text ?? ""} />;
    case "plain":
    case "text":
    case undefined:
    case null:
      return <TextPart text={part.text ?? ""} />;
    default:
      // Unknown kind: try to fall back to text content, otherwise nothing.
      return part.text ? <TextPart text={part.text} /> : <></>;
  }
};

/**
 * Render an ordered list of typed message parts plus an optional streaming
 * cursor. The renderer is purposely defensive: unknown kinds collapse to
 * plain text, missing fields are ignored.
 */
export const MessagePartsRenderer: Component<MessagePartsRendererProps> = (props) => {
  return (
    <div class={["message-parts", props.class].filter(Boolean).join(" ")}>
      <Show
        when={props.parts && props.parts.length > 0}
        fallback={
          <Show when={props.fallbackText}>
            <MarkdownPart text={props.fallbackText!} />
          </Show>
        }
      >
        <For each={props.parts}>{(part) => <PartRouter part={part} />}</For>
      </Show>
      <Show when={props.streaming}>
        <CursorPart />
      </Show>
    </div>
  );
};

export default MessagePartsRenderer;
