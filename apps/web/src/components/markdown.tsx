import ReactMarkdown from "react-markdown"

/**
 * 有限 Markdown 渲染：禁用原始 HTML（react-markdown 默认不渲染），
 * 链接仅放行 http/https，其余协议一律剥离。
 */
const urlTransform = (url: string) => {
  if (/^(https?:\/\/|mailto:)/i.test(url)) return url
  if (url.startsWith("/") || url.startsWith("#")) return url
  return ""
}

export function Markdown({ children }: { children: string }) {
  return (
    <div className="markdown-body text-sm leading-relaxed break-words">
      <ReactMarkdown
        urlTransform={urlTransform}
        components={{
          a: ({ href, children }) => (
            <a
              href={href}
              target="_blank"
              rel="noreferrer noopener nofollow"
              className="text-primary underline underline-offset-2"
            >
              {children}
            </a>
          ),
          code: ({ children }) => (
            <code className="bg-muted rounded px-1 py-0.5 font-mono text-xs">{children}</code>
          ),
          pre: ({ children }) => (
            <pre className="bg-muted overflow-x-auto rounded-lg p-3 text-xs">{children}</pre>
          ),
          img: () => null,
        }}
      >
        {children}
      </ReactMarkdown>
    </div>
  )
}
