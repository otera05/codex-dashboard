import { useEffect, useRef, useState } from "react";
import { Archive, Check, Clipboard, ExternalLink, Folder, MoreHorizontal, Pencil, RotateCcw } from "lucide-react";
import type { Session } from "../types";
import { openDirectory } from "../lib/bridge";

export function SessionHeaderMenu({ session, archived, onAction }: { session: Session; archived: boolean; onAction: (action: "rename" | "archive" | "restore", session: Session) => void }) {
  const [open, setOpen] = useState(false);
  const [copied, setCopied] = useState<"id" | "cwd">();
  const rootRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const close = (event: MouseEvent | KeyboardEvent) => {
      if (event instanceof KeyboardEvent && event.key !== "Escape") return;
      if (event instanceof MouseEvent && rootRef.current?.contains(event.target as Node)) return;
      setOpen(false);
    };
    window.addEventListener("mousedown", close);
    window.addEventListener("keydown", close);
    return () => { window.removeEventListener("mousedown", close); window.removeEventListener("keydown", close); };
  }, [open]);

  const copy = async (kind: "id" | "cwd", value: string) => {
    await navigator.clipboard.writeText(value);
    setCopied(kind);
    window.setTimeout(() => setCopied(undefined), 1200);
  };

  const act = (action: "rename" | "archive" | "restore") => {
    setOpen(false);
    onAction(action, session);
  };

  return <div className="header-menu-root" ref={rootRef}>
    <button className="icon-button" aria-label="Session options" aria-expanded={open} onClick={() => setOpen((current) => !current)}><MoreHorizontal size={18} /></button>
    {open && <div className="header-session-menu" role="menu">
      {!archived && <button role="menuitem" onClick={() => act("rename")}><Pencil size={14} /> Rename</button>}
      {archived ? <button role="menuitem" onClick={() => act("restore")}><RotateCcw size={14} /> Restore</button> : <button role="menuitem" onClick={() => act("archive")}><Archive size={14} /> Archive</button>}
      <span />
      <button role="menuitem" onClick={() => void copy("id", session.id)}>{copied === "id" ? <Check size={14} /> : <Clipboard size={14} />} {copied === "id" ? "Copied session ID" : "Copy session ID"}</button>
      <button role="menuitem" onClick={() => void copy("cwd", session.cwd)}>{copied === "cwd" ? <Check size={14} /> : <Folder size={14} />} {copied === "cwd" ? "Copied directory" : "Copy directory"}</button>
      <button role="menuitem" onClick={() => { setOpen(false); void openDirectory(session.cwd); }}><ExternalLink size={14} /> Open directory</button>
    </div>}
  </div>;
}
