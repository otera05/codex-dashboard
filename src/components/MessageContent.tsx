import { Check, Copy } from "lucide-react";
import { Children, isValidElement, type ReactNode, useState } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";

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

function MarkdownPre({ children }: { children?: ReactNode }) {
  const child = Children.toArray(children)[0];
  if (!isValidElement<{ className?: string; children?: ReactNode }>(child)) return <pre>{children}</pre>;

  const language = child.props.className?.match(/language-([\w+-]+)/)?.[1];
  const code = String(child.props.children ?? "").replace(/\n$/, "");
  return <CodeBlock code={code} language={language} />;
}

export function MessageContent({ text }: { text: string }) {
  return <div className="message-content">
    <ReactMarkdown
      remarkPlugins={[remarkGfm]}
      components={{
        pre: MarkdownPre,
        code: ({ className, children, ...props }) => <code className={["inline-code", className].filter(Boolean).join(" ")} {...props}>{children}</code>,
        a: ({ children, ...props }) => <a {...props} target="_blank" rel="noreferrer">{children}</a>,
      }}
    >{text}</ReactMarkdown>
  </div>;
}
