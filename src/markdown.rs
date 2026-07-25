use pulldown_cmark::{html, CodeBlockKind, CowStr, Event, Options, Parser, Tag, TagEnd};

pub fn render_markdown(markdown: &str) -> String {
    render(markdown)
}

pub fn render_streaming_markdown(markdown: &str) -> String {
    let has_open_fence = open_fence(markdown).is_some();
    let preview = complete_streaming_tail(markdown);
    let mut output = render(&preview);
    if has_open_fence {
        mark_last_code_block_streaming(&mut output);
    }
    output
}

fn render(markdown: &str) -> String {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_GFM;
    let mut events = Parser::new_ext(markdown, options).filter_map(move |event| match event {
        Event::Start(Tag::HtmlBlock) => None,
        Event::End(TagEnd::HtmlBlock) => None,
        Event::Html(html) | Event::InlineHtml(html) => Some(Event::Text(html)),
        Event::Start(Tag::Image {
            link_type,
            dest_url,
            title,
            id,
        }) => Some(Event::Start(Tag::Link {
            link_type,
            dest_url: safe_https_url(&dest_url),
            title,
            id,
        })),
        Event::End(TagEnd::Image) => Some(Event::End(TagEnd::Link)),
        Event::Start(Tag::Link {
            link_type,
            dest_url,
            title,
            id,
        }) => Some(Event::Start(Tag::Link {
            link_type,
            dest_url: safe_https_url(&dest_url),
            title,
            id,
        })),
        Event::Start(Tag::CodeBlock(kind)) => {
            let safe_kind = match kind {
                CodeBlockKind::Indented => CodeBlockKind::Indented,
                CodeBlockKind::Fenced(info) => {
                    CodeBlockKind::Fenced(CowStr::Boxed(safe_code_info(&info).into_boxed_str()))
                }
            };
            Some(Event::Start(Tag::CodeBlock(safe_kind)))
        }
        other => Some(other),
    });

    let mut output = String::new();
    html::push_html(&mut output, &mut events);
    output
}

fn safe_https_url(url: &str) -> CowStr<'static> {
    let url = url.trim();
    let Some(prefix) = url.get(..8) else {
        return CowStr::Borrowed("");
    };
    if !prefix.eq_ignore_ascii_case("https://") {
        return CowStr::Borrowed("");
    }
    let rest = &url[8..];
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let host = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    if authority.contains('@') || host.is_empty() || host.chars().any(char::is_whitespace) {
        return CowStr::Borrowed("");
    }
    CowStr::Boxed(url.to_owned().into_boxed_str())
}

fn safe_code_info(info: &str) -> String {
    info.split_whitespace()
        .next()
        .unwrap_or_default()
        .chars()
        .filter(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
        .take(32)
        .collect()
}

fn complete_streaming_tail(markdown: &str) -> String {
    if markdown.is_empty() {
        return String::new();
    }

    let mut output = markdown.to_owned();
    if let Some(fence) = open_fence(markdown) {
        if !output.ends_with('\n') {
            output.push('\n');
        }
        output.push_str(&fence);
        output.push('\n');
        return output;
    }

    let tail_start = markdown
        .rfind("\n\n")
        .map_or(0, |separator| separator.saturating_add(2));
    let tail = &markdown[tail_start..];
    let backtick_len = unmatched_backtick_run(tail);
    if backtick_len > 0 {
        output.push_str(&"`".repeat(backtick_len));
        return output;
    }

    let (strong, strike) = unmatched_high_confidence_delimiters(tail);
    if strike {
        output.push_str("~~");
    }
    if strong {
        output.push_str("**");
    }
    output
}

fn open_fence(markdown: &str) -> Option<String> {
    let mut open: Option<(char, usize)> = None;
    for line in markdown.lines() {
        let trimmed = line.trim_start_matches([' ', '\t', '>']);
        let Some(marker) = trimmed.chars().next() else {
            continue;
        };
        if !matches!(marker, '`' | '~') {
            continue;
        }
        let count = trimmed
            .chars()
            .take_while(|character| *character == marker)
            .count();
        if count < 3 {
            continue;
        }
        match open {
            Some((open_marker, open_count))
                if marker == open_marker
                    && count >= open_count
                    && trimmed[count..].trim().is_empty() =>
            {
                open = None;
            }
            None => open = Some((marker, count)),
            _ => {}
        }
    }
    open.map(|(marker, count)| std::iter::repeat_n(marker, count).collect())
}

fn mark_last_code_block_streaming(html: &mut String) {
    if let Some(start) = html.rfind("<pre>") {
        html.replace_range(
            start..start + "<pre>".len(),
            "<pre class=\"streaming-open\">",
        );
    }
}

fn unmatched_backtick_run(tail: &str) -> usize {
    let mut open = 0;
    let bytes = tail.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'\\' {
            index = index.saturating_add(2);
            continue;
        }
        if bytes[index] != b'`' {
            index += 1;
            continue;
        }
        let start = index;
        while index < bytes.len() && bytes[index] == b'`' {
            index += 1;
        }
        let length = index - start;
        if length >= 3 {
            continue;
        }
        if open == length {
            open = 0;
        } else if open == 0 {
            open = length;
        }
    }
    open
}

fn unmatched_high_confidence_delimiters(tail: &str) -> (bool, bool) {
    let bytes = tail.as_bytes();
    let mut strong = false;
    let mut strike = false;
    let mut in_code = false;
    let mut index = 0;
    while index + 1 < bytes.len() {
        if bytes[index] == b'\\' {
            index = index.saturating_add(2);
            continue;
        }
        if bytes[index] == b'`' {
            in_code = !in_code;
            index += 1;
            continue;
        }
        if in_code {
            index += 1;
            continue;
        }
        if &bytes[index..index + 2] == b"**" {
            strong = !strong;
            index += 2;
            continue;
        }
        if &bytes[index..index + 2] == b"~~" {
            strike = !strike;
            index += 2;
            continue;
        }
        index += 1;
    }
    (strong, strike)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_html_is_escaped() {
        let rendered = render_markdown("<script>alert(1)</script>");
        assert!(!rendered.contains("<script>"));
        assert!(rendered.contains("&lt;script&gt;"));
    }

    #[test]
    fn unsafe_links_are_inert() {
        assert!(render_markdown("[bad](javascript:alert(1))").contains("href=\"\""));
        assert!(render_markdown("[bad](https://user@example.com/path)").contains("href=\"\""));
        assert!(render_markdown("[good](https://example.com/path)")
            .contains("https://example.com/path"));
    }

    #[test]
    fn images_do_not_load_remote_resources() {
        let rendered = render_markdown("![설명](https://example.com/image.png)");
        assert!(!rendered.contains("<img"));
        assert!(rendered.contains("<a href=\"https://example.com/image.png\""));
    }

    #[test]
    fn streaming_strong_is_previewed_without_changing_source() {
        let source = "**빛나는 표본";
        let rendered = render_streaming_markdown(source);
        assert!(rendered.contains("<strong>빛나는 표본</strong>"));
        assert_eq!(source, "**빛나는 표본");
    }

    #[test]
    fn streaming_inline_code_is_previewed() {
        let rendered = render_streaming_markdown("명령은 `cargo check");
        assert!(rendered.contains("<code>cargo check</code>"));
    }

    #[test]
    fn streaming_fence_is_previewed_and_marked() {
        let rendered = render_streaming_markdown("```rust\nfn main() {");
        assert!(rendered.contains("language-rust"));
        assert!(rendered.contains("<pre class=\"streaming-open\">"));
    }
}
