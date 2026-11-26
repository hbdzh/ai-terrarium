import { ref } from "vue";
import type MarkdownItType from "markdown-it";
import type { HLJSApi } from "highlight.js";
import type katexType from "katex";

let mdInstance: MarkdownItType | null = null;
let mdPromise: Promise<void> | null = null;
let hljsInstance: HLJSApi | null = null;
let katexInstance: typeof katexType | null = null;
let hljsPromise: Promise<void> | null = null;
let katexPromise: Promise<void> | null = null;

/** Incremented when async markdown-it, syntax-highlighting, or KaTeX modules finish loading. */
export const markdownEnhancersVersion = ref(0);

const MAX_RENDER_CACHE = 300;
const renderedMarkdownCache = new Map<string, string>();
let renderedCacheVersion = -1;

export function warmupMarkdownParser(): void {
  ensureMdLoaded();
}

function escapeHtmlBasic(str: string): string {
  return str
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function ensureMdLoaded(): void {
  if (mdInstance || mdPromise) return;
  mdPromise = import("markdown-it")
    .then((mod) => {
      const MarkdownIt = mod.default;
      const instance = new MarkdownIt({
        html: false,
        linkify: true,
        breaks: true,
        highlight(str: string, lang: string): string {
          if (!hljsInstance) {
            ensureHljsLoaded();
            return `<pre class="hljs"><code>${instance.utils.escapeHtml(str)}</code></pre>`;
          }
          if (lang && hljsInstance.getLanguage(lang)) {
            try {
              return `<pre class="hljs"><code>${hljsInstance.highlight(str, { language: lang, ignoreIllegals: true }).value}</code></pre>`;
            } catch {
              // fallback below
            }
          }
          return `<pre class="hljs"><code>${instance.utils.escapeHtml(str)}</code></pre>`;
        },
      });
      mdInstance = instance;
      markdownEnhancersVersion.value += 1;
    })
    .catch(() => {
      // Fallback to basic escaped rendering on load failure
    });
}

function ensureHljsLoaded(): void {
  if (hljsInstance || hljsPromise) return;
  hljsPromise = Promise.all([
    import("highlight.js/lib/common"),
    import("highlight.js/styles/github.css"),
  ])
    .then(([mod]) => {
      hljsInstance = mod.default;
      markdownEnhancersVersion.value += 1;
    })
    .catch(() => {
      // Fallback to plain escaped code blocks on load failure
    });
}

function ensureKatexLoaded(): void {
  if (katexInstance || katexPromise) return;
  katexPromise = Promise.all([
    import("katex"),
    import("katex/dist/katex.min.css"),
  ])
    .then(([mod]) => {
      katexInstance = mod.default;
      markdownEnhancersVersion.value += 1;
    })
    .catch(() => {
      // Fallback to raw math text on load failure
    });
}

/**
 * Auto-closes unclosed fenced code blocks (``` or ~~~) and unclosed display math ($$)
 * during live token-by-token streaming so layout never flickers or breaks.
 */
export function stabilizeStreamingMarkdown(raw: string): string {
  let result = raw;

  // Count triple-backtick code fences
  const fenceMatches = result.match(/(^|\n)[ \t]*```/g);
  if (fenceMatches && fenceMatches.length % 2 === 1) {
    result += "\n```";
  }

  // Only check unclosed $$ outside closed code blocks
  const withoutCode = result.replace(/```[\s\S]*?```/g, "");
  const displayMathMatches = withoutCode.match(/\$\$/g);
  if (displayMathMatches && displayMathMatches.length % 2 === 1) {
    result += "\n$$";
  }

  return result;
}

/**
 * Renders Markdown with KaTeX math ($...$ and $$...$$) and syntax-highlighted code blocks.
 * Protects fenced/inline code before substituting math delimiters.
 */
export function renderMarkdown(raw: string, isStreaming = false): string {
  // Subscribe reactive templates so they re-render once markdown-it/hljs/katex finish lazy-loading
  const ver = markdownEnhancersVersion.value;

  if (!isStreaming) {
    if (renderedCacheVersion !== ver) {
      renderedMarkdownCache.clear();
      renderedCacheVersion = ver;
    }
    const cached = renderedMarkdownCache.get(raw);
    if (cached !== undefined) {
      // Refresh LRU recency
      renderedMarkdownCache.delete(raw);
      renderedMarkdownCache.set(raw, cached);
      return cached;
    }
  }

  const input = isStreaming ? stabilizeStreamingMarkdown(raw) : raw;

  if (!mdInstance) {
    ensureMdLoaded();
    return `<p>${escapeHtmlBasic(input).replace(/\n/g, "<br>")}</p>`;
  }
  const md = mdInstance;

  // Temporarily mask fenced code blocks and inline code so math regex never touches code
  const codePlaceholders: string[] = [];
  let masked = input.replace(/```[\s\S]*?```|`[^`\n]+`/g, (match) => {
    const idx = codePlaceholders.length;
    codePlaceholders.push(match);
    return `@@CODE_BLOCK_${idx}@@`;
  });

  const mathHtmlPlaceholders: string[] = [];

  const renderMathExpr = (expr: string, displayMode: boolean, rawFallback: string): string => {
    if (!katexInstance) {
      ensureKatexLoaded();
      return md.utils.escapeHtml(rawFallback);
    }
    try {
      return katexInstance.renderToString(expr.trim(), {
        displayMode,
        throwOnError: false,
      });
    } catch {
      return md.utils.escapeHtml(rawFallback);
    }
  };

  // Block math $$...$$
  masked = masked.replace(/\$\$([\s\S]+?)\$\$/g, (full, expr: string) => {
    const idx = mathHtmlPlaceholders.length;
    mathHtmlPlaceholders.push(renderMathExpr(expr, true, full));
    return `@@MATH_HTML_${idx}@@`;
  });

  // Inline math $...$ (avoid matching currency like $100)
  masked = masked.replace(
    /(?<!\\)\$(?!\s)([^$\n]+?)(?<!\s)\$/g,
    (full, expr: string) => {
      const idx = mathHtmlPlaceholders.length;
      mathHtmlPlaceholders.push(renderMathExpr(expr, false, full));
      return `@@MATH_HTML_${idx}@@`;
    }
  );

  // Restore code blocks before markdown-it parses
  const restoredForMd = masked.replace(/@@CODE_BLOCK_(\d+)@@/g, (_, idx) => {
    return codePlaceholders[Number(idx)] ?? "";
  });

  let html = md.render(restoredForMd);

  // Restore rendered KaTeX HTML
  html = html.replace(/@@MATH_HTML_(\d+)@@/g, (_, idx) => {
    return mathHtmlPlaceholders[Number(idx)] ?? "";
  });

  if (!isStreaming) {
    if (renderedMarkdownCache.size >= MAX_RENDER_CACHE) {
      const oldestKey = renderedMarkdownCache.keys().next().value;
      if (oldestKey !== undefined) {
        renderedMarkdownCache.delete(oldestKey);
      }
    }
    renderedMarkdownCache.set(raw, html);
  }

  return html;
}
