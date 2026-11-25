/// 流式与历史思考链净化器（移植自 DeepSeek Harness 思路）
/// 负责：
/// 1. 实时识别流式正文中的 `<think>...</think>` 标签（支持跨分片截断）
/// 2. 剥离历史消息中的推理思考过程，确保发往模型的上下文纯净无污染

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SanitizedSegment {
    Reasoning(String),
    Content(String),
}

/// 有状态流式 `<think>` 标签拆分器。
/// 能够正确处理 `<think>` 或 `</think>` 在多个 SSE chunk 之间被截断的情况。
#[derive(Debug, Default)]
pub struct ThinkTagStreamParser {
    in_think: bool,
    pending: String,
    content_started: bool,
}

const OPEN_TAG: &str = "<think>";
const CLOSE_TAG: &str = "</think>";

impl ThinkTagStreamParser {
    pub fn new() -> Self {
        Self::default()
    }

    /// 推入一段来自 SSE `delta.content` 的增量文本，返回拆分后的思考/正文片段。
    pub fn push(&mut self, chunk: &str) -> Vec<SanitizedSegment> {
        if chunk.is_empty() {
            return Vec::new();
        }
        self.pending.push_str(chunk);
        self.drain_ready(false)
    }

    /// 在流结束时刷新剩余缓冲区。
    pub fn finish(&mut self) -> Vec<SanitizedSegment> {
        self.drain_ready(true)
    }

    fn drain_ready(&mut self, is_eof: bool) -> Vec<SanitizedSegment> {
        let mut out = Vec::new();

        loop {
            if self.pending.is_empty() {
                break;
            }

            let target_tag = if self.in_think { CLOSE_TAG } else { OPEN_TAG };
            let lower = self.pending.to_ascii_lowercase();

            if let Some(byte_pos) = lower.find(target_tag) {
                // `target_tag` 由纯 ASCII 构成，因此 `lower` 中的字节偏移与 `self.pending` 安全对齐
                let before = self.pending[..byte_pos].to_string();
                let after_idx = byte_pos + target_tag.len();
                self.pending = self.pending[after_idx..].to_string();

                if !before.is_empty() {
                    self.emit_segment(&mut out, before);
                }
                self.in_think = !self.in_think;
                continue;
            }

            if is_eof {
                let rest = std::mem::take(&mut self.pending);
                if !rest.is_empty() {
                    self.emit_segment(&mut out, rest);
                }
                break;
            }

            // 检查末尾是否存在与 target_tag 前缀部分匹配的子串，保留至下一次 chunk
            let hold_bytes = longest_partial_tag_suffix(&lower, target_tag);
            let safe_len = self.pending.len().saturating_sub(hold_bytes);
            if safe_len == 0 {
                break;
            }
            let safe_part = self.pending[..safe_len].to_string();
            self.pending = self.pending[safe_len..].to_string();
            if !safe_part.is_empty() {
                self.emit_segment(&mut out, safe_part);
            }
            break;
        }

        out
    }

    fn emit_segment(&mut self, out: &mut Vec<SanitizedSegment>, mut text: String) {
        if self.in_think {
            out.push(SanitizedSegment::Reasoning(text));
        } else {
            if !self.content_started {
                let trimmed = text.trim_start_matches(['\r', '\n']);
                if trimmed.is_empty() {
                    return;
                }
                if trimmed.len() != text.len() {
                    text = trimmed.to_string();
                }
                self.content_started = true;
            }
            out.push(SanitizedSegment::Content(text));
        }
    }
}

fn longest_partial_tag_suffix(haystack_lower: &str, tag_lower: &str) -> usize {
    let max_check = haystack_lower.len().min(tag_lower.len().saturating_sub(1));
    for len in (1..=max_check).rev() {
        let start = haystack_lower.len() - len;
        if haystack_lower.is_char_boundary(start) && haystack_lower[start..] == tag_lower[..len] {
            return len;
        }
    }
    0
}

/// 彻底剥离文本中的所有 `<think>...</think>` 区块（含未闭合尾部标签），
/// 用于在组装发往任何模型 API 的历史消息时进行上下文净化。
pub fn strip_think_tags(raw: &str) -> String {
    let mut parser = ThinkTagStreamParser::new();
    let mut cleaned = String::with_capacity(raw.len());
    for seg in parser.push(raw) {
        if let SanitizedSegment::Content(c) = seg {
            cleaned.push_str(&c);
        }
    }
    for seg in parser.finish() {
        if let SanitizedSegment::Content(c) = seg {
            cleaned.push_str(&c);
        }
    }
    cleaned.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_think_parser_handles_split_tags_across_chunks() {
        let mut p = ThinkTagStreamParser::new();
        let chunks = ["<thi", "nk>先想", "一下", "</th", "ink>\n\n正式回答内容"];
        let mut reasoning = String::new();
        let mut content = String::new();

        for c in chunks {
            for seg in p.push(c) {
                match seg {
                    SanitizedSegment::Reasoning(r) => reasoning.push_str(&r),
                    SanitizedSegment::Content(s) => content.push_str(&s),
                }
            }
        }
        for seg in p.finish() {
            match seg {
                SanitizedSegment::Reasoning(r) => reasoning.push_str(&r),
                SanitizedSegment::Content(s) => content.push_str(&s),
            }
        }

        assert_eq!(reasoning, "先想一下");
        assert_eq!(content, "正式回答内容");
    }

    #[test]
    fn test_think_parser_handles_utf8_boundaries() {
        let mut p = ThinkTagStreamParser::new();
        let mut content = String::new();
        for c in ["你好", "世界<", "think>内心戏</think>继续说"] {
            for seg in p.push(c) {
                if let SanitizedSegment::Content(s) = seg {
                    content.push_str(&s);
                }
            }
        }
        for seg in p.finish() {
            if let SanitizedSegment::Content(s) = seg {
                content.push_str(&s);
            }
        }
        assert_eq!(content, "你好世界继续说");
    }

    #[test]
    fn test_strip_think_tags_cleans_history() {
        let raw = "<THINK>隐秘推理</THINK>\n\n这是公开观点。<think>补充想法</think>结尾。";
        assert_eq!(strip_think_tags(raw), "这是公开观点。结尾。");
    }
}
