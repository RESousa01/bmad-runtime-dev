import { useMemo, useState } from "react";
import { OpenCodeIcon } from "./OpenCodeIcon";
import { projectContextUsage } from "../features/model-context/contextUsage";
import {
  unmeasuredModelContext,
  type ModelContextState,
} from "../features/model-context/modelContextTypes";
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
  /**
   * The locally derived input figure, when there is one.
   *
   * Byte-derived on every path that supplies it -- a review's
   * `totalTokenEstimate` or the attachment sum -- so it is an estimate, and
   * `projectContextUsage` is what decides whether it is still the best number
   * available or a measurement has superseded it.
   */
  readonly inputTokens?: number | undefined;
  /**
   * What the host measured for this task's last assembled request.
   *
   * The same projection the Context surface reads, deliberately: this panel
   * showed a "Review Budget Used" percentage against a constant 65,536 -- the
   * workspace broker's file-read cap -- and a second, independently-written
   * answer to "how much context is in use" is how one surface comes to
   * contradict the other.
   */
  readonly modelContext?: ModelContextState | undefined;
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
  modelContext = unmeasuredModelContext,
  messages,
  model,
  outputTokens,
  provider,
  reasoningTokens,
  sessionTitle,
}: SessionContextPanelProps) {
  const userMessages = messages.filter((message) => message.role === "user").length;
  const assistantMessages = messages.filter((message) => message.role === "assistant").length;
  /*
   * The same projection the Context surface reads.
   *
   * It owns the two questions this panel used to answer for itself and get
   * wrong: which figure is authoritative, and whether there is a window to
   * measure it against. Answering them here as well is how the drawer and the
   * composer come to disagree about the same session.
   */
  const usage = projectContextUsage(modelContext, inputTokens ?? 0);
  const measured = usage.basis === "reported";
  /*
   * A measurement supersedes the local estimate rather than joining it. The
   * host measures the whole assembled request, attachments included, so adding
   * the byte-derived figure on top would count the same files twice.
   */
  const inputContribution = measured
    ? usage.usedTokens
    : inputTokens;
  const knownTokens = [inputContribution, outputTokens, reasoningTokens]
    .filter((value): value is number => value !== undefined);
  const totalTokens = knownTokens.length === 0
    ? undefined
    : knownTokens.reduce((total, value) => total + value, 0);
  /*
   * The total inherits the estimate-ness of its parts.
   *
   * Output and reasoning are counts the provider reported. The input half is
   * only a count once the host has measured it -- otherwise it is the
   * byte-derived figure the row below calls "Input Estimate". Summing them is
   * useful; printing the sum under a bare "Total Tokens" is not, because the
   * label launders a guess into a measurement while the one row that admits
   * the guess sits directly beneath it saying otherwise.
   */
  const totalLabel = measured || inputContribution === undefined
    ? "Total Tokens"
    : "Total Tokens (input estimated)";
  /*
   * Only when a window is actually known.
   *
   * This is the row that used to read "Local Review Budget 65,536". The host
   * exposes no model context window anywhere -- `ModelAuthStatusProjection`
   * carries no token count, `ContextPreviewProjection.modelTarget` is typed as
   * the literal `null`, and the only `context_window` in the Rust host is a
   * profile hash -- so absent is the truthful state, and the row is absent
   * rather than showing a placeholder that reads like a measurement.
   */
  const windowRows: readonly (readonly [string, string])[] =
    usage.window.kind === "known"
      ? [
        ["Model Context Window", formatNumber(usage.window.windowTokens)],
        ["Context Used", `${(usage.window.fraction * 100).toFixed(1)}%`],
      ]
      : [];
  const lastActivity = messages.reduce<number | undefined>((latest, message) => {
    if (message.createdAt === undefined) return latest;
    return latest === undefined ? message.createdAt : Math.max(latest, message.createdAt);
  }, undefined);
  const stats: readonly (readonly [string, string])[] = [
    ["Session", sessionTitle],
    ["Messages", formatNumber(messages.length)],
    ["Provider", provider || "—"],
    ["Model", model || "—"],
    /*
     * "Local Review Budget" and "Review Budget Used" are gone from this list.
     *
     * The budget was a constant 65,536 -- the workspace broker's file-read cap
     * -- and the percentage was this session's tokens over it. Neither is a
     * property of the model, so the percentage answered a question nobody
     * asked while looking exactly like the answer to the one they did: how
     * much of the model's context is left. The rows above answer that question
     * when it can be answered, and are absent when it cannot.
     */
    ...windowRows,
    [totalLabel, formatNumber(totalTokens)],
    /*
     * The input row says which it is.
     *
     * "Input Estimate" was honest while the figure was byte-derived, and would
     * be a lie the moment a measurement arrived under the same label. `stale`
     * is named rather than folded into "reported": a measurement the route has
     * moved out from under is still the best number available, and its age is
     * the thing that decides whether to trust it.
     */
    [
      measured
        ? usage.stale ? "Input (last reported)" : "Input (reported)"
        : "Input Estimate",
      formatNumber(inputContribution),
    ],
    ["Output Tokens", formatNumber(outputTokens)],
    ["Reasoning Tokens", formatNumber(reasoningTokens)],
    ["Cache Tokens (read/write)", `${formatNumber(cacheReadTokens)} / ${formatNumber(cacheWriteTokens)}`],
    ["User Messages", formatNumber(userMessages)],
    ["Assistant Messages", formatNumber(assistantMessages)],
    ["Total Cost", formatCost(costUsd)],
    ["Session Created", formatTime(createdAt)],
    ["Last Activity", formatTime(lastActivity)],
  ];

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
