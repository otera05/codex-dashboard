import { Ban, Check, CheckCircle2, Copy, LoaderCircle, Terminal, XCircle } from "lucide-react";
import { useState } from "react";
import type { CommandActivity as CommandActivityItem } from "../types";

const statusDetails = {
  inProgress: { label: "Running", icon: LoaderCircle },
  completed: { label: "Completed", icon: CheckCircle2 },
  failed: { label: "Failed", icon: XCircle },
  declined: { label: "Declined", icon: Ban },
} as const;

function formatDuration(durationMs?: number) {
  if (durationMs == null) return null;
  return durationMs < 1_000 ? `${durationMs}ms` : `${(durationMs / 1_000).toFixed(1)}s`;
}

export function CommandActivity({ item }: { item: CommandActivityItem }) {
  const [copied, setCopied] = useState(false);
  const status = statusDetails[item.status];
  const StatusIcon = status.icon;
  const duration = formatDuration(item.durationMs);
  const lineCount = item.output?.split("\n").length ?? 0;

  const copyOutput = async () => {
    if (!item.output) return;
    try {
      await navigator.clipboard.writeText(item.output);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1_500);
    } catch {
      setCopied(false);
    }
  };

  return <article className={`command-activity ${item.status}`}>
    <div className="command-heading">
      <span className="command-icon"><Terminal size={15} /></span>
      <strong>Command</strong>
      <span className="command-status"><StatusIcon size={13} /> {status.label}</span>
      {duration && <span className="command-duration">{duration}</span>}
    </div>
    <div className="command-line"><span>$</span><code>{item.command}</code></div>
    <div className="command-meta"><span>{item.cwd}</span>{item.exitCode != null && <span>exit {item.exitCode}</span>}</div>
    {item.output && <details className="command-output">
      <summary><span>Output</span><small>{lineCount} {lineCount === 1 ? "line" : "lines"}</small></summary>
      <div className="command-output-body">
        <button type="button" onClick={() => void copyOutput()} aria-label="Copy command output">{copied ? <Check size={13} /> : <Copy size={13} />}{copied ? "Copied" : "Copy"}</button>
        <pre>{item.output}</pre>
      </div>
    </details>}
  </article>;
}
