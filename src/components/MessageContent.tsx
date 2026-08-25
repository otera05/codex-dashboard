import { Check, Copy } from "lucide-react";
import { Fragment, useState } from "react";

interface ContentPart {
  type: "text" | "code";
  value: string;
  language?: string;
}

export function parseMessageContent(text: string): ContentPart[] {
  const parts: ContentPart[] = [];
  const fence = /```([^\n`]*)\r?\n([\s\S]*?)```/g;
  let cursor = 0;
  let match: RegExpExecArray | null;

  while ((match = fence.exec(text)) !== null) {
    if (match.index > cursor) parts.push({ type: "text", value: text.slice(cursor, match.index) });
    parts.push({
      type: "code",
      language: match[1].trim() || undefined,
      value: match[2].replace(/\r\n/g, "\n").replace(/\n$/, ""),
    });
    cursor = match.index + match[0].length;
  }

  if (cursor < text.length) parts.push({ type: "text", value: text.slice(cursor) });
  return parts.length ? parts : [{ type: "text", value: text }];
}

function CodeBlock({ code, language }: { code: string; language?: string }) {
  const [copied, setCopied] = useState(false);

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(code);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1_500);
    } catch {
      setCopied(false);
    }
  };

  return <div className="code-block">
    <div className="code-block-header">
      <span>{language ?? "code"}</span>
      <button type="button" onClick={() => void copy()} aria-label="Copy code">
        {copied ? <Check size={13} /> : <Copy size={13} />}{copied ? "Copied" : "Copy"}
      </button>
    </div>
    <pre><code className={language ? `language-${language}` : undefined}>{code}</code></pre>
  </div>;
}

export function MessageContent({ text }: { text: string }) {
  return <div className="message-content">
    {parseMessageContent(text).map((part, index) => <Fragment key={`${part.type}-${index}`}>
      {part.type === "code" ? <CodeBlock code={part.value} language={part.language} /> : part.value && <p>{part.value}</p>}
    </Fragment>)}
  </div>;
}
