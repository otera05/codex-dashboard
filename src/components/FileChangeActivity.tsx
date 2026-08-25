import { Ban, Check, CheckCircle2, Copy, FileDiff, LoaderCircle, XCircle } from "lucide-react";
import { useState } from "react";
import type { FileChange, FileChangeActivity as FileChangeActivityItem } from "../types";

const statusDetails = {
  inProgress: { label: "Applying", icon: LoaderCircle },
  completed: { label: "Completed", icon: CheckCircle2 },
  failed: { label: "Failed", icon: XCircle },
  declined: { label: "Declined", icon: Ban },
} as const;

function lineKind(line: string) {
  if (line.startsWith("+++") || line.startsWith("---") || line.startsWith("diff ") || line.startsWith("index ")) return "meta";
  if (line.startsWith("+")) return "added";
  if (line.startsWith("-")) return "deleted";
  if (line.startsWith("@@")) return "hunk";
  return "context";
}

function FileDiffView({ change, open }: { change: FileChange; open: boolean }) {
  const [copied, setCopied] = useState(false);
  const copyDiff = async () => {
    try {
      await navigator.clipboard.writeText(change.diff);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1_500);
    } catch {
      setCopied(false);
    }
  };

  return <details className="file-change-entry" open={open}>
    <summary className="file-change-summary">
      <span className={`file-kind ${change.kind}`}>{change.kind}</span>
      <code className="file-path">{change.path}{change.movePath && <> → {change.movePath}</>}</code>
      <span className="diff-stats"><b>+{change.additions}</b><i>−{change.deletions}</i></span>
    </summary>
    <div className="file-diff">
      <div className="diff-toolbar"><span>Unified diff</span><button type="button" onClick={() => void copyDiff()} aria-label={`Copy diff for ${change.path}`}>{copied ? <Check size={12} /> : <Copy size={12} />}{copied ? "Copied" : "Copy"}</button></div>
      {change.diff ? <pre className="diff-content">{change.diff.split("\n").map((line, index) => <span className={`diff-line ${lineKind(line)}`} key={`${index}:${line}`}>{line || " "}</span>)}</pre> : <div className="empty-diff">No diff available</div>}
    </div>
  </details>;
}

export function FileChangeActivity({ item }: { item: FileChangeActivityItem }) {
  const status = statusDetails[item.status];
  const StatusIcon = status.icon;
  const additions = item.changes.reduce((total, change) => total + change.additions, 0);
  const deletions = item.changes.reduce((total, change) => total + change.deletions, 0);

  return <article className={`file-change-activity ${item.status}`}>
    <div className="file-change-heading">
      <span className="file-change-icon"><FileDiff size={15} /></span>
      <strong>File changes</strong>
      <span className="file-change-count">{item.changes.length} {item.changes.length === 1 ? "file" : "files"}</span>
      <span className="file-change-total"><b>+{additions}</b><i>−{deletions}</i></span>
      <span className="file-change-status"><StatusIcon size={13} /> {status.label}</span>
    </div>
    <div className="file-change-list">{item.changes.map((change, index) => <FileDiffView change={change} open={item.changes.length === 1} key={`${change.path}:${index}`} />)}</div>
  </article>;
}
