import { useMemo, useState } from "react";
import { OpenCodeIcon } from "./OpenCodeIcon";
import "./session-context-panel.css";

/**
 * React adaptation of OpenCode's session-context-tab.tsx.
 * Source: library/opencode/packages/app/src/components/session/session-context-tab.tsx
 * License: MIT. See THIRD_PARTY_NOTICES.md.
 */
export interface SessionContextMessage {
  readonly createdAt?: number | undefined;
  readonly id: string;
  readonly role: "assistant" | "system" | "tool" | "user";
  readonly value: unknown;
}

export interface SessionContextPanelProps {
  readonly cacheReadTokens?: number | undefined;
  readonly cacheWriteTokens?: number | undefined;
  readonly costUsd?: number | undefined;
  readonly createdAt?: number | undefined;
  readonly inputTokens?: number | undefined;
  readonly localReviewBudget?: number | undefined;
  readonly messages: readonly SessionContextMessage[];
  readonly model?: string | undefined;
  readonly outputTokens?: number | undefined;
  readonly provider?: string | undefined;
  readonly reasoningTokens?: number | undefined;
  readonly sessionTitle: string;
}

function formatNumber(value: number | undefined): string {
  return value === undefined ? "—" : new Intl.NumberFormat().format(value);
}

function formatCost(value: number | undefined): string {
  return value === undefined
    ? "—"
    : new Intl.NumberFormat(undefined, { style: "currency", currency: "USD" }).format(value);
}

function formatTime(value: number | undefined): string {
  if (value === undefined) return "—";
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(new Date(value));
}

function JsonLine({ line, number }: { readonly line: string; readonly number: number }) {
  const tokens = line.split(/("(?:\\.|[^"\\])*")(?=\s*:)|("(?:\\.|[^"\\])*")|\b(true|false|null|-?\d+(?:\.\d+)?)\b/gu);
  return (
    <span className="raw-message__line">
      <span aria-hidden="true" className="raw-message__line-number">{number}</span>
      <span className="raw-message__line-code">
        {tokens.map((token, index) => {
          if (!token) return null;
          const rest = line.slice(line.indexOf(token) + token.length);
          const isQuoted = token.startsWith('"');
          const isKey = isQuoted && /^\s*:/u.test(rest);
          const kind = isKey
            ? "key"
            : isQuoted
              ? "string"
              : /^(?:true|false|null|-?\d)/u.test(token)
                ? "literal"
                : "plain";
          return <span className={`raw-message__token raw-message__token--${kind}`} key={`${index}-${token}`}>{token}</span>;
        })}
      </span>
    </span>
  );
}

function RawMessage({ message }: { readonly message: SessionContextMessage }) {
  const [expanded, setExpanded] = useState(false);
  const json = useMemo(() => JSON.stringify(message.value, null, 2) ?? "null", [message.value]);
  return (
    <article className="raw-message">
      <button
        aria-expanded={expanded}
        aria-label={`${message.role} • ${message.id}`}
        className="raw-message__trigger"
        onClick={() => setExpanded((value) => !value)}
        type="button"
      >
        <span><strong>{message.role}</strong> <span>• {message.id}</span></span>
        <span className="raw-message__meta">
          <time>{formatTime(message.createdAt)}</time>
          <OpenCodeIcon className="raw-message__grabber" name="chevron-grabber-vertical" size={14} />
        </span>
      </button>
      {expanded ? (
        <pre className="raw-message__code" tabIndex={0}>
          <code>{json.split("\n").map((line, index) => (
            <JsonLine key={index} line={line} number={index + 1} />
          ))}</code>
        </pre>
      ) : null}
    </article>
  );
}

function Stat({ label, value }: { readonly label: string; readonly value: string }) {
  return (
    <div className="session-context__stat">
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

export function SessionContextPanel({
  cacheReadTokens,
  cacheWriteTokens,
  costUsd,
  createdAt,
  inputTokens,
  localReviewBudget,
  messages,
  model,
  outputTokens,
  provider,
  reasoningTokens,
  sessionTitle,
}: SessionContextPanelProps) {
  const userMessages = messages.filter((message) => message.role === "user").length;
  const assistantMessages = messages.filter((message) => message.role === "assistant").length;
  const knownTokens = [inputTokens, outputTokens, reasoningTokens]
    .filter((value): value is number => value !== undefined);
  const totalTokens = knownTokens.length === 0
    ? undefined
    : knownTokens.reduce((total, value) => total + value, 0);
  const usage = localReviewBudget && totalTokens !== undefined
    ? `${Math.min(100, (totalTokens / localReviewBudget) * 100).toFixed(1)}%`
    : "—";
  const lastActivity = messages.reduce<number | undefined>((latest, message) => {
    if (message.createdAt === undefined) return latest;
    return latest === undefined ? message.createdAt : Math.max(latest, message.createdAt);
  }, undefined);
  const stats = [
    ["Session", sessionTitle],
    ["Messages", formatNumber(messages.length)],
    ["Provider", provider || "—"],
    ["Model", model || "—"],
    ["Local Review Budget", formatNumber(localReviewBudget)],
    ["Total Tokens", formatNumber(totalTokens)],
    ["Review Budget Used", usage],
    ["Input Estimate", formatNumber(inputTokens)],
    ["Output Tokens", formatNumber(outputTokens)],
    ["Reasoning Tokens", formatNumber(reasoningTokens)],
    ["Cache Tokens (read/write)", `${formatNumber(cacheReadTokens)} / ${formatNumber(cacheWriteTokens)}`],
    ["User Messages", formatNumber(userMessages)],
    ["Assistant Messages", formatNumber(assistantMessages)],
    ["Total Cost", formatCost(costUsd)],
    ["Session Created", formatTime(createdAt)],
    ["Last Activity", formatTime(lastActivity)],
  ] as const;

  return (
    <section className="session-context" data-component="session-context" data-source="opencode">
      <div className="session-context__scroll">
        <div className="session-context__stats">
          {stats.map(([label, value]) => <Stat key={label} label={label} value={value} />)}
        </div>
        <div className="session-context__raw">
          <div className="session-context__section-title">Raw messages</div>
          {messages.length === 0 ? (
            <div className="session-context__empty">
              <OpenCodeIcon name="status" size={16} />
              No raw messages in this session yet.
            </div>
          ) : messages.map((message) => <RawMessage key={message.id} message={message} />)}
        </div>
      </div>
    </section>
  );
}
