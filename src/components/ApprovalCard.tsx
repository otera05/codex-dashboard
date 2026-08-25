import { FileWarning, ShieldAlert, Terminal } from "lucide-react";
import { useState } from "react";
import { resolveApproval } from "../lib/bridge";
import type { ApprovalRequest } from "../types";

type Decision = "accept" | "acceptForSession" | "decline";

export function ApprovalCard({ approval }: { approval: ApprovalRequest }) {
  const [responding, setResponding] = useState<Decision>();
  const [error, setError] = useState<string>();
  const isCommand = approval.kind === "command";
  const canAcceptForSession = approval.availableDecisions.includes("acceptForSession");

  const respond = async (decision: Decision) => {
    setResponding(decision);
    setError(undefined);
    try {
      await resolveApproval(approval.requestId, decision);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
      setResponding(undefined);
    }
  };

  return <article className="approval-card">
    <div className="approval-heading">
      <span className="approval-icon"><ShieldAlert size={16} /></span>
      <div><strong>Approval required</strong><span>{isCommand ? "Command execution" : "File changes"}</span></div>
      <span className="approval-pending">Waiting</span>
    </div>
    <div className="approval-content">
      {isCommand ? <div className="approval-command"><Terminal size={14} /><code>{approval.command || "Command details unavailable"}</code></div> : <div className="approval-command"><FileWarning size={14} /><code>{approval.grantRoot ? `Write access: ${approval.grantRoot}` : "Apply the proposed file changes"}</code></div>}
      {approval.reason && <p>{approval.reason}</p>}
      {approval.cwd && <span className="approval-cwd">Working directory: {approval.cwd}</span>}
      {error && <div className="approval-error">{error}</div>}
    </div>
    <div className="approval-actions">
      <button className="approval-decline" disabled={Boolean(responding)} onClick={() => void respond("decline")}>{responding === "decline" ? "Declining…" : "Decline"}</button>
      {canAcceptForSession && <button disabled={Boolean(responding)} onClick={() => void respond("acceptForSession")}>{responding === "acceptForSession" ? "Approving…" : "Allow for session"}</button>}
      <button className="approval-accept" disabled={Boolean(responding)} onClick={() => void respond("accept")}>{responding === "accept" ? "Approving…" : "Approve once"}</button>
    </div>
  </article>;
}
