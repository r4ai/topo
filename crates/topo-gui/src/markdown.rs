//! What the notes editor knows about Markdown: the colors of its syntax, and
//! how Enter and Tab treat lists and indentation.
//!
//! Everything here is a pure function of the text. Highlighting only says how
//! to color the bytes that are there, never which bytes are there, so the
//! cursor, the selection and an input method's marked text stay where they are.

use std::ops::Range;

/// How a stretch of the text is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    Plain,
    Heading,
    Emphasis,
    Strong,
    /// Inline code and the lines of a fenced code block.
    Code,
    Link,
    /// List markers, quote marks and code fences.
    Marker,
    Quote,
}

/// `len` bytes drawn in one `style`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    pub len: usize,
    pub style: Style,
}

/// One indentation step.
const INDENT: &str = "  ";

/// Splits `text` into spans that cover all of it, each in the style its Markdown calls for.
pub fn highlight(text: &str) -> Vec<Span> {
    let mut spans = Spans::default();
    let mut fence: Option<char> = None;
    for (index, line) in text.split('\n').enumerate() {
        if index > 0 {
            spans.push(1, Style::Plain);
        }
        match (fence, fence_char(line)) {
            (None, Some(open)) => {
                fence = Some(open);
                spans.push(line.len(), Style::Marker);
            }
            (Some(open), Some(close)) if open == close && line.trim().chars().all(|c| c == open) => {
                fence = None;
                spans.push(line.len(), Style::Marker);
            }
            (Some(_), _) => spans.push(line.len(), Style::Code),
            (None, None) if is_heading(line) => spans.push(line.len(), Style::Heading),
            (None, None) => block(line, &mut spans),
        }
    }
    spans.0
}

/// Spans that merge with the previous one of the same style.
#[derive(Default)]
struct Spans(Vec<Span>);

impl Spans {
    fn push(&mut self, len: usize, style: Style) {
        match self.0.last_mut() {
            _ if len == 0 => {}
            Some(last) if last.style == style => last.len += len,
            _ => self.0.push(Span { len, style }),
        }
    }
}

/// The character of the code fence `line` opens or closes, if it is one.
fn fence_char(line: &str) -> Option<char> {
    let line = line.strip_prefix("   ").or(line.strip_prefix("  ")).or(line.strip_prefix(' ')).unwrap_or(line);
    ['`', '~'].into_iter().find(|c| line.starts_with(&c.to_string().repeat(3)))
}

fn is_heading(line: &str) -> bool {
    let line = line.trim_start_matches(' ');
    let marks = line.bytes().take_while(|b| *b == b'#').count();
    (1..=6).contains(&marks) && matches!(line[marks..].chars().next(), None | Some(' ' | '\t'))
}

/// A line that is neither a heading nor part of a code block.
fn block(line: &str, spans: &mut Spans) {
    let prefix = Prefix::of(line);
    spans.push(prefix.indent, Style::Plain);
    spans.push(prefix.quote - prefix.indent, Style::Marker);
    spans.push(prefix.list_start - prefix.quote, Style::Plain);
    spans.push(prefix.list_end - prefix.list_start, Style::Marker);
    let base = if prefix.quote > prefix.indent { Style::Quote } else { Style::Plain };
    inline(&line[prefix.list_end..], base, spans);
}

/// Emphasis, code and links inside a line of text.
fn inline(text: &str, base: Style, spans: &mut Spans) {
    let (mut at, mut plain) = (0, 0);
    while at < text.len() {
        let rest = &text[at..];
        let step = match rest.as_bytes()[0] {
            b'\\' => rest.chars().take(2).map(char::len_utf8).sum(),
            b'`' => match code_span(rest) {
                Some(len) => {
                    spans.push(at - plain, base);
                    spans.push(len, Style::Code);
                    (at, plain) = (at + len, at + len);
                    continue;
                }
                None => rest.bytes().take_while(|b| *b == b'`').count(),
            },
            _ => match span_at(text, at) {
                Some((len, style)) => {
                    spans.push(at - plain, base);
                    spans.push(len, style);
                    (at, plain) = (at + len, at + len);
                    continue;
                }
                None => rest.chars().next().map_or(1, char::len_utf8),
            },
        };
        at += step;
    }
    spans.push(text.len() - plain, base);
}

/// The length of the code span `rest` starts with: from its opening run of
/// backticks to the next run of as many.
fn code_span(rest: &str) -> Option<usize> {
    let ticks = rest.bytes().take_while(|b| *b == b'`').count();
    let mut at = ticks;
    while let Some(found) = rest[at..].find('`') {
        let start = at + found;
        let run = rest[start..].bytes().take_while(|b| *b == b'`').count();
        if run == ticks {
            return Some(start + run);
        }
        at = start + run;
    }
    None
}

/// The link, emphasis or strong text that starts at byte `at` of `text`, as its length and style.
fn span_at(text: &str, at: usize) -> Option<(usize, Style)> {
    let rest = &text[at..];
    let before = text[..at].chars().next_back();
    match rest.as_bytes()[0] {
        b'[' | b'!' => link(rest).map(|len| (len, Style::Link)),
        b'h' if before.is_none_or(|c| !c.is_alphanumeric()) => url(rest).map(|len| (len, Style::Link)),
        c @ (b'*' | b'_') => {
            let delimiter = if rest.as_bytes().get(1) == Some(&c) { 2 } else { 1 };
            if c == b'_' && before.is_some_and(char::is_alphanumeric) {
                return None;
            }
            let (open, body) = rest.split_at(delimiter);
            if body.chars().next().is_none_or(char::is_whitespace) {
                return None;
            }
            let (mut from, mut found) = (0, None);
            while let Some(i) = body[from..].find(open) {
                let end = from + i;
                let (prev, next) = (body[..end].chars().next_back(), body[end + delimiter..].chars().next());
                let lone = delimiter == 2 || (prev != Some(c as char) && next != Some(c as char));
                let word_end = c != b'_' || next.is_none_or(|n| !n.is_alphanumeric());
                if end > 0 && lone && word_end && prev.is_some_and(|p| !p.is_whitespace()) {
                    found = Some(end);
                    break;
                }
                from = end + delimiter.max(1);
            }
            let style = if delimiter == 2 { Style::Strong } else { Style::Emphasis };
            found.map(|end| (delimiter + end + delimiter, style))
        }
        _ => None,
    }
}

/// The length of the `[text](target)` (or `![alt](target)`) `rest` starts with.
fn link(rest: &str) -> Option<usize> {
    let bracket = rest.strip_prefix('!').unwrap_or(rest);
    let start = rest.len() - bracket.len();
    let label = bracket.strip_prefix('[')?;
    let close = label.find(']')?;
    let target = label[close + 1..].strip_prefix('(')?;
    let end = target.find(')')?;
    Some(start + 1 + close + 1 + 1 + end + 1)
}

/// The length of the `http://` or `https://` address `rest` starts with.
fn url(rest: &str) -> Option<usize> {
    let scheme = ["https://", "http://"].into_iter().find(|s| rest.starts_with(s))?;
    let len = rest.find(char::is_whitespace).unwrap_or(rest.len());
    let address = rest[..len].trim_end_matches(['.', ',', ';', ':', '!', '?', ')', ']', '>']);
    (address.len() > scheme.len()).then_some(address.len())
}

/// What a line starts with, as byte offsets into it: indentation, block quote
/// marks and a list marker.
struct Prefix {
    /// End of the indentation.
    indent: usize,
    /// End of the `>` marks; the indent when there are none.
    quote: usize,
    /// Where the list marker starts and ends, counting its checkbox and the space after it.
    list_start: usize,
    list_end: usize,
    /// The marker of the next item of the list.
    next: String,
}

impl Prefix {
    fn of(line: &str) -> Self {
        let indent = line.len() - line.trim_start_matches([' ', '\t']).len();
        let mut quote = indent;
        while line[quote..].starts_with('>') {
            quote += 1;
            quote += usize::from(line[quote..].starts_with(' '));
        }
        let list_start = quote + line[quote..].len() - line[quote..].trim_start_matches(' ').len();
        let rest = &line[list_start..];
        let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
        let ordered = (1..=9).contains(&digits)
            && matches!(rest.as_bytes().get(digits), Some(b'.' | b')'))
            && rest.as_bytes().get(digits + 1) == Some(&b' ');
        let (mut len, mut next) = match rest.as_bytes() {
            [b'-' | b'*' | b'+', b' ', ..] => (2, rest[..2].to_owned()),
            _ if ordered => {
                let number: u64 = rest[..digits].parse().expect("digits are a number");
                (digits + 2, format!("{}{} ", number + 1, &rest[digits..=digits]))
            }
            _ => (0, String::new()),
        };
        if len > 0 && ["[ ] ", "[x] ", "[X] "].iter().any(|b| rest[len..].starts_with(b)) {
            len += 4;
            next.push_str("[ ] ");
        }
        Self { indent, quote, list_start, list_end: list_start + len, next }
    }

    /// The end of everything before the text of the line.
    fn end(&self) -> usize {
        self.list_end.max(self.quote)
    }
}

/// An edit to the text: the bytes `range` become `text`, and `selection` is
/// what is selected afterwards, in the new text.
#[derive(Debug, PartialEq, Eq)]
pub struct Change {
    pub range: Range<usize>,
    pub text: String,
    pub selection: Range<usize>,
}

/// Enter: a line break that carries on the indentation, list or quote of the line.
/// On an item with nothing in it the marker is taken away instead, which ends the list.
pub fn newline(text: &str, selection: &Range<usize>) -> Change {
    let plain = |text: &str| Change {
        range: selection.clone(),
        text: text.to_owned(),
        selection: selection.start + text.len()..selection.start + text.len(),
    };
    if !selection.is_empty() {
        return plain("\n");
    }
    let cursor = selection.start;
    let (start, end) = (line_start(text, cursor), line_end(text, cursor));
    let line = &text[start..end];
    let prefix = Prefix::of(line);
    // Inside the marker there is nothing to continue; only the indentation is.
    if cursor - start < prefix.end() {
        let kept = prefix.indent.min(cursor - start);
        return plain(&format!("\n{}", &line[..kept]));
    }
    if prefix.end() > prefix.indent && line[prefix.end()..].trim().is_empty() {
        return Change { range: start..end, text: String::new(), selection: start..start };
    }
    plain(&format!("\n{}{}", &line[..prefix.list_start], prefix.next))
}

/// Tab: indents the lines the selection touches, or types an indentation when
/// the selection is inside one line that is not a list item. `None` when there
/// is no line with anything to indent.
pub fn indent(text: &str, selection: &Range<usize>) -> Option<Change> {
    let first = line_start(text, selection.start);
    let item = Prefix::of(&text[first..line_end(text, first)]);
    if !text[selection.clone()].contains('\n') && item.list_end == item.list_start {
        let at = selection.start + INDENT.len();
        return Some(Change { range: selection.clone(), text: INDENT.to_owned(), selection: at..at });
    }
    reindent(text, selection, |line| (!line.is_empty()).then_some((0, INDENT.to_owned())))
}

/// Shift-Tab: takes one indentation off every line the selection touches.
/// `None` when none of them has any.
pub fn outdent(text: &str, selection: &Range<usize>) -> Option<Change> {
    reindent(text, selection, |line| {
        let spaces = line.bytes().take_while(|b| *b == b' ').count().min(INDENT.len());
        match (spaces, line.starts_with('\t')) {
            (0, true) => Some((1, String::new())),
            (0, false) => None,
            (spaces, _) => Some((spaces, String::new())),
        }
    })
}

/// Rewrites the start of each line the selection touches. `edit` gives, for a
/// line, how many bytes to remove from its start and what to put there.
fn reindent(text: &str, selection: &Range<usize>, edit: impl Fn(&str) -> Option<(usize, String)>) -> Option<Change> {
    let (first, last) = (line_start(text, selection.start), touched_end(text, selection));
    // Each edit: where in the old text, how much it removes and what it inserts.
    let mut edits = Vec::new();
    let mut at = first;
    for line in text[first..last].split('\n') {
        edits.extend(edit(line).map(|(removed, added)| (at, removed, added)));
        at += line.len() + 1;
    }
    if edits.is_empty() {
        return None;
    }
    let mut replaced = String::new();
    let mut from = first;
    for (at, removed, added) in &edits {
        replaced.push_str(&text[from..*at]);
        replaced.push_str(added);
        from = at + removed;
    }
    replaced.push_str(&text[from..last]);
    // A position moves by what was added before it, and sticks to the edit that removed it.
    let moved = |point: usize| {
        let mut shift = 0isize;
        for (at, removed, added) in &edits {
            if point >= at + removed {
                shift += added.len() as isize - *removed as isize;
            } else if point > *at {
                shift -= (point - at) as isize;
            }
        }
        point.checked_add_signed(shift).expect("an edit never moves a position before the text")
    };
    Some(Change { range: first..last, text: replaced, selection: moved(selection.start)..moved(selection.end) })
}

/// The end of the last line a selection touches. A selection that ends at the
/// start of a line does not touch that line.
fn touched_end(text: &str, selection: &Range<usize>) -> usize {
    match selection.is_empty() || selection.end != line_start(text, selection.end) {
        true => line_end(text, selection.end),
        false => selection.end - 1,
    }
}

/// Start of the line `offset` is on.
pub fn line_start(text: &str, offset: usize) -> usize {
    text[..offset].rfind('\n').map_or(0, |at| at + 1)
}

/// End of the line `offset` is on, before its line break.
pub fn line_end(text: &str, offset: usize) -> usize {
    text[offset..].find('\n').map_or(text.len(), |at| offset + at)
}

#[cfg(test)]
mod tests {
    use super::*;
    use Style::*;

    /// The text of each span with its style, skipping the plain ones' line breaks.
    fn styled(text: &str) -> Vec<(&str, Style)> {
        let mut at = 0;
        highlight(text)
            .into_iter()
            .map(|span| {
                at += span.len;
                (&text[at - span.len..at], span.style)
            })
            .collect()
    }

    #[test]
    fn spans_cover_the_text_exactly() {
        let text = "# Title\n\n- *one* **two** `three`\n> quote [a](b)\n```rs\nlet _x = 1;\n```\nhttps://x.dev/a.\n日本語 *強調* `コード`";
        let total: usize = highlight(text).iter().map(|span| span.len).sum();
        assert_eq!(total, text.len());
        assert_eq!(highlight(""), []);
    }

    #[test]
    fn headings_are_whole_lines() {
        assert_eq!(
            styled("# One\ntext\n###### Six\n####### no"),
            [("# One", Heading), ("\ntext\n", Plain), ("###### Six", Heading), ("\n####### no", Plain)]
        );
        // A hash needs a space after it.
        assert_eq!(styled("#tag"), [("#tag", Plain)]);
        assert_eq!(styled("#"), [("#", Heading)]);
    }

    #[test]
    fn emphasis_and_strong() {
        assert_eq!(
            styled("a *b* **c** _d_ __e__ f"),
            [
                ("a ", Plain),
                ("*b*", Emphasis),
                (" ", Plain),
                ("**c**", Strong),
                (" ", Plain),
                ("_d_", Emphasis),
                (" ", Plain),
                ("__e__", Strong),
                (" f", Plain)
            ]
        );
        // Neither a lone star around spaces nor an underscore inside a word starts one.
        assert_eq!(styled("2 * 3 * 4 snake_case_name *open"), [("2 * 3 * 4 snake_case_name *open", Plain)]);
        // It does not run past the line.
        assert_eq!(styled("*a\nb*"), [("*a\nb*", Plain)]);
    }

    #[test]
    fn inline_code_wins_over_what_is_inside_it() {
        assert_eq!(
            styled("`a *b*` ``x ` y`` `open"),
            [("`a *b*`", Code), (" ", Plain), ("``x ` y``", Code), (" `open", Plain)]
        );
    }

    #[test]
    fn fenced_blocks_are_code_until_the_closing_fence() {
        assert_eq!(
            styled("a\n```rust\n# not *a* heading\n```\n*b*"),
            [
                ("a\n", Plain),
                ("```rust", Marker),
                ("\n", Plain),
                ("# not *a* heading", Code),
                ("\n", Plain),
                ("```", Marker),
                ("\n", Plain),
                ("*b*", Emphasis)
            ]
        );
        // A block that is never closed runs to the end; ~~~ does not close ```.
        assert_eq!(
            styled("```\nx\n~~~\ny"),
            [("```", Marker), ("\n", Plain), ("x", Code), ("\n", Plain), ("~~~", Code), ("\n", Plain), ("y", Code)]
        );
    }

    #[test]
    fn links_and_addresses() {
        assert_eq!(
            styled("see [the doc](https://a.b/c) and ![i](x.png) or https://a.b/c_d, ok"),
            [
                ("see ", Plain),
                ("[the doc](https://a.b/c)", Link),
                (" and ", Plain),
                ("![i](x.png)", Link),
                (" or ", Plain),
                ("https://a.b/c_d", Link),
                (", ok", Plain)
            ]
        );
        assert_eq!(styled("[not a link] (x) http://"), [("[not a link] (x) http://", Plain)]);
    }

    #[test]
    fn list_and_quote_markers() {
        assert_eq!(
            styled("- a\n  1. b *c*\n* [x] d\n> q **s**\n-no"),
            [
                ("- ", Marker),
                ("a\n  ", Plain),
                ("1. ", Marker),
                ("b ", Plain),
                ("*c*", Emphasis),
                ("\n", Plain),
                ("* [x] ", Marker),
                ("d\n", Plain),
                ("> ", Marker),
                ("q ", Quote),
                ("**s**", Strong),
                ("\n-no", Plain)
            ]
        );
    }

    #[test]
    fn escapes_and_multibyte_text_keep_their_bytes() {
        assert_eq!(styled(r"\*a\* *é*"), [(r"\*a\* ", Plain), ("*é*", Emphasis)]);
        assert_eq!(styled("日本*語*`コ`"), [("日本", Plain), ("*語*", Emphasis), ("`コ`", Code)]);
    }

    fn typed(text: &str, at: usize) -> (String, Range<usize>) {
        let change = newline(text, &(at..at));
        let result = format!("{}{}{}", &text[..change.range.start], change.text, &text[change.range.end..]);
        (result, change.selection)
    }

    #[test]
    fn enter_continues_lists_quotes_and_indentation() {
        assert_eq!(typed("- a", 3), ("- a\n- ".into(), 6..6));
        assert_eq!(typed("  * a", 5), ("  * a\n  * ".into(), 10..10));
        assert_eq!(typed("9. a", 4), ("9. a\n10. ".into(), 9..9));
        assert_eq!(typed("- [x] a", 7), ("- [x] a\n- [ ] ".into(), 14..14));
        assert_eq!(typed("> a", 3), ("> a\n> ".into(), 6..6));
        assert_eq!(typed("  text", 6), ("  text\n  ".into(), 9..9));
        assert_eq!(typed("text", 4), ("text\n".into(), 5..5));
        // In the middle of an item the rest of it moves to the new one.
        assert_eq!(typed("- ab", 3), ("- a\n- b".into(), 6..6));
        // Inside the indentation or the marker only the indentation up to the cursor continues.
        assert_eq!(typed("  - a", 1), (" \n  - a".into(), 3..3));
        assert_eq!(typed("-no", 3), ("-no\n".into(), 4..4));
    }

    #[test]
    fn enter_on_an_empty_item_ends_the_list() {
        assert_eq!(typed("- a\n- ", 6), ("- a\n".into(), 4..4));
        assert_eq!(typed("1. a\n  2. ", 10), ("1. a\n".into(), 5..5));
        assert_eq!(typed("> ", 2), ("".into(), 0..0));
        assert_eq!(typed("- [ ] ", 6), ("".into(), 0..0));
    }

    #[test]
    fn enter_over_a_selection_replaces_it() {
        let change = newline("- abc", &(3..5));
        assert_eq!((change.range, change.text.as_str(), change.selection), (3..5, "\n", 4..4));
    }

    fn apply(text: &str, change: &Change) -> String {
        format!("{}{}{}", &text[..change.range.start], change.text, &text[change.range.end..])
    }

    #[test]
    fn tab_types_an_indentation_or_indents_lines() {
        // Inside a plain line it is typed at the cursor.
        let change = indent("ab", &(1..1)).unwrap();
        assert_eq!((apply("ab", &change).as_str(), change.selection), ("a  b", 3..3));
        // On a list item it indents the item, the cursor staying in the text.
        let change = indent("- a\n- b", &(7..7)).unwrap();
        assert_eq!((apply("- a\n- b", &change).as_str(), change.selection), ("- a\n  - b", 9..9));
        // Several lines are all indented, blank ones left alone, and the selection follows.
        let text = "a\n\nb\nc";
        let change = indent(text, &(0..4)).unwrap();
        assert_eq!((apply(text, &change).as_str(), change.selection), ("  a\n\n  b\nc", 2..8));
        // A selection that ends at the start of a line does not touch it.
        let change = indent("a\nb", &(0..2)).unwrap();
        assert_eq!(apply("a\nb", &change), "  a\nb");
        assert_eq!(indent("\n\n", &(0..2)), None);
    }

    #[test]
    fn shift_tab_takes_one_step_off_each_line() {
        let text = "    a\n b\n\tc\nd";
        let change = outdent(text, &(0..text.len())).unwrap();
        assert_eq!(apply(text, &change), "  a\nb\nc\nd");
        assert_eq!(change.selection, 0..text.len() - 4);
        // The cursor sticks to the start of the line when its spaces go.
        let change = outdent("  ab", &(1..1)).unwrap();
        assert_eq!((apply("  ab", &change).as_str(), change.selection), ("ab", 0..0));
        assert_eq!(outdent("ab\ncd", &(0..5)), None);
    }

    #[test]
    fn lines_are_found_between_line_breaks() {
        let text = "ab\n\ncd";
        assert_eq!([0, 1, 2].map(|at| (line_start(text, at), line_end(text, at))), [(0, 2), (0, 2), (0, 2)]);
        assert_eq!((line_start(text, 3), line_end(text, 3)), (3, 3));
        assert_eq!((line_start(text, 6), line_end(text, 6)), (4, 6));
    }
}
