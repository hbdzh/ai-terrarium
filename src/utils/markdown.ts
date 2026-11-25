import MarkdownIt from "markdown-it";
import hljs from "highlight.js";
import katex from "katex";

const md = new MarkdownIt({
  html: false,
  linkify: true,
  breaks: true,
  highlight(str: string, lang: string): string {
    if (lang && hljs.getLanguage(lang)) {
      try {
        return `<pre class="hljs"><code>${hljs.highlight(str, { language: lang, ignoreIllegals: true }).value}</code></pre>`;
      } catch {
        // fallback below
      }
    }
    return `<pre class="hljs"><code>${md.utils.escapeHtml(str)}</code></pre>`;
  },
});

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
  const input = isStreaming ? stabilizeStreamingMarkdown(raw) : raw;

  // Temporarily mask fenced code blocks and inline code so math regex never touches code
  const codePlaceholders: string[] = [];
  let masked = input.replace(/```[\s\S]*?```|`[^`\n]+`/g, (match) => {
    const idx = codePlaceholders.length;
    codePlaceholders.push(match);
    return `@@CODE_BLOCK_${idx}@@`;
  });

  const mathHtmlPlaceholders: string[] = [];

  // Block math $$...$$
  masked = masked.replace(/\$\$([\s\S]+?)\$\$/g, (_, expr: string) => {
    const idx = mathHtmlPlaceholders.length;
    try {
      mathHtmlPlaceholders.push(
        katex.renderToString(expr.trim(), {
          displayMode: true,
          throwOnError: false,
        })
      );
    } catch {
      mathHtmlPlaceholders.push(`$$${expr}$$`);
    }
    return `@@MATH_HTML_${idx}@@`;
  });

  // Inline math $...$ (avoid matching currency like $100)
  masked = masked.replace(
    /(?<!\\)\$(?!\s)([^$\n]+?)(?<!\s)\$/g,
    (_, expr: string) => {
      const idx = mathHtmlPlaceholders.length;
      try {
        mathHtmlPlaceholders.push(
          katex.renderToString(expr.trim(), {
            displayMode: false,
            throwOnError: false,
          })
        );
      } catch {
        mathHtmlPlaceholders.push(`$${expr}$`);
      }
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

  return html;
}
