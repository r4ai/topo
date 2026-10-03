//! Editing one property of the selected node in place, in its row of the inspector.
//!
//! Enter saves, Escape cancels, and so does a click elsewhere or anything else
//! that moves the focus: a half-typed value is never saved by accident. Input that cannot be
//! saved keeps the field open and says why under it.

use gpui::{Context, Entity, Focusable, Window};
use jiff::civil::Date;
use topo_core::model::{normalize_assignee, normalize_pr, pr_label};
use topo_core::{Edit, Node, NodeId, Priority};

use crate::text_input::{InputEvent, TextInput};
use crate::{TopoApp, dates, parse_tags, theme};

#[derive(Clone, Copy, PartialEq, Eq, Debug, clap::ValueEnum)]
pub(crate) enum Field {
    Priority,
    Assignee,
    Due,
    Tags,
    /// A pull request to add to the node's list.
    Pr,
}

/// The field being edited and the node it belongs to.
pub(crate) struct InlineEdit {
    pub node: NodeId,
    pub field: Field,
    /// Why the last Enter saved nothing.
    pub error: Option<String>,
}

impl Field {
    fn placeholder(self) -> &'static str {
        match self {
            Field::Priority => "low, medium, high, urgent — empty clears",
            Field::Assignee => "Name or agent — empty clears",
            Field::Due => "2026-10-31, 10-31, +3d, tomorrow — empty clears",
            Field::Tags => "Space-separated tags — empty clears",
            Field::Pr => "URL or owner/repo#123",
        }
    }

    /// The text the field opens with: the current value.
    fn initial(self, node: &Node) -> String {
        match self {
            Field::Priority => node.priority.map(|p| p.to_string()).unwrap_or_default(),
            Field::Assignee => node.assignee.clone().unwrap_or_default(),
            Field::Due => node.due.map(|d| d.to_string()).unwrap_or_default(),
            Field::Tags => node.tags.join(" "),
            Field::Pr => String::new(),
        }
    }

    /// The change to `node` that the typed `text` asks for.
    fn parse(self, text: &str, node: &Node, today: Date) -> Result<Edit, String> {
        let text = text.trim();
        let edit = match self {
            Field::Priority => Edit { priority: Some(parse_priority(text)?), ..Edit::default() },
            Field::Assignee => {
                let assignee = match text {
                    "" => None,
                    name => Some(normalize_assignee(name).map_err(|e| e.to_string())?),
                };
                Edit { assignee: Some(assignee), ..Edit::default() }
            }
            Field::Due => Edit { due: Some(dates::parse_due(text, today)?), ..Edit::default() },
            Field::Tags => Edit { tags: Some(parse_tags(text)), ..Edit::default() },
            Field::Pr => {
                let url = normalize_pr(text).map_err(|e| e.to_string())?;
                if node.prs.contains(&url) {
                    return Err(format!("{} is already linked", pr_label(&url)));
                }
                Edit { prs: Some(node.prs.iter().cloned().chain([url]).collect()), ..Edit::default() }
            }
        };
        Ok(edit)
    }

    /// What Enter would save, shown under the field while typing, and its color.
    pub(crate) fn preview(self, text: &str, node: &Node, today: Date) -> (String, u32) {
        let edit = match self.parse(text, node, today) {
            Ok(edit) => edit,
            Err(_) if self == Field::Due => return ("Not a date yet".into(), theme::FAINT),
            Err(_) if text.trim().is_empty() => return (self.placeholder().into(), theme::FAINT),
            Err(e) => return (e, theme::FAINT),
        };
        let saved = match self {
            Field::Priority => edit.priority.flatten().map(|p| theme::priority_label(p).to_owned()),
            Field::Assignee => edit.assignee.flatten().map(|a| format!("@{a}")),
            Field::Due => {
                edit.due.flatten().map(|d| format!("{} · {} ({})", d.strftime("%a"), d, dates::relative(d, today)))
            }
            Field::Tags => edit.tags.filter(|t| !t.is_empty()).map(|t| format!("#{}", t.join(" #"))),
            Field::Pr => edit.prs.and_then(|prs| prs.last().map(|url| pr_label(url))),
        };
        match saved {
            Some(value) => (format!("→ {value}"), theme::GREEN),
            None => ("→ None".into(), theme::MUTED),
        }
    }
}

/// A priority by its name or an unambiguous start of it; empty is no priority.
fn parse_priority(text: &str) -> Result<Option<Priority>, String> {
    let text = text.to_lowercase();
    if text.is_empty() {
        return Ok(None);
    }
    match Priority::ALL.into_iter().filter(|p| p.to_string().starts_with(&text)).collect::<Vec<_>>()[..] {
        [priority] => Ok(Some(priority)),
        _ => Err(format!("`{text}` is not a priority (low, medium, high or urgent)")),
    }
}

impl TopoApp {
    /// Whether `field` of `node` is being edited in place.
    pub(crate) fn editing(&self, node: &NodeId, field: Field) -> Option<&InlineEdit> {
        self.inline.as_ref().filter(|edit| edit.node == *node && edit.field == field)
    }

    pub(crate) fn inline_input(&self) -> &Entity<TextInput> {
        &self.inline_input
    }

    /// Turns the row of `field` of the selected node into a text field.
    pub(crate) fn start_inline(&mut self, field: Field, window: &mut Window, cx: &mut Context<Self>) {
        let Some(node) = self.selected_node() else { return };
        let (id, initial) = (node.id.clone(), field.initial(node));
        self.inline_input.update(cx, |input, cx| input.reset(&initial, field.placeholder(), cx));
        self.inline = Some(InlineEdit { node: id, field, error: None });
        self.show_help = false;
        window.focus(&self.inline_input.focus_handle(cx), cx);
        cx.notify();
    }

    /// Closes the field without saving and returns the keyboard to the canvas.
    pub(crate) fn cancel_inline(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.inline.take().is_some() {
            window.focus(&self.focus, cx);
            cx.notify();
        }
    }

    fn submit_inline(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(InlineEdit { node, field, .. }) = &self.inline else { return };
        let (id, field) = (node.clone(), *field);
        let Some(node) = self.graph().get(&id) else { return self.cancel_inline(window, cx) };
        let text = self.inline_input.read(cx).text().to_owned();
        // Nothing typed adds no pull request.
        if field == Field::Pr && text.trim().is_empty() {
            return self.cancel_inline(window, cx);
        }
        match field.parse(&text, node, dates::today()) {
            Ok(edit) => {
                if self.mutate(cx, |graph| graph.edit(&id, edit)) {
                    self.cancel_inline(window, cx);
                }
            }
            Err(error) => {
                self.inline.as_mut().expect("checked above").error = Some(error);
                cx.notify();
            }
        }
    }

    pub(crate) fn on_inline_event(
        &mut self,
        _: &Entity<TextInput>,
        event: &InputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            InputEvent::Changed => {
                if let Some(edit) = &mut self.inline {
                    edit.error = None;
                }
            }
            InputEvent::Cancel => self.cancel_inline(window, cx),
            InputEvent::Submit => self.submit_inline(window, cx),
            InputEvent::Up | InputEvent::Down => {}
        }
        cx.notify();
    }

    /// Removes one pull request from `id`.
    pub(crate) fn unlink_pr(&mut self, id: NodeId, url: &str, cx: &mut Context<Self>) {
        let Some(node) = self.graph().get(&id) else { return };
        let prs = node.prs.iter().filter(|pr| *pr != url).cloned().collect();
        self.mutate(cx, |graph| graph.edit(&id, Edit { prs: Some(prs), ..Edit::default() }));
    }
}

#[cfg(test)]
mod tests {
    use topo_core::Kind;

    use super::*;

    #[test]
    fn fields_parse_what_was_typed_or_say_why_not() {
        let today = jiff::civil::date(2026, 10, 3);
        let mut node = Node::new(NodeId("a".into()), Kind::Task, "a".into());
        node.prs = vec!["https://github.com/o/r/pull/1".into()];
        let parse = |field: Field, text: &str| field.parse(text, &node, today);

        assert_eq!(parse(Field::Priority, "H").unwrap().priority, Some(Some(Priority::High)));
        assert_eq!(parse(Field::Priority, " ").unwrap().priority, Some(None));
        assert!(parse(Field::Priority, "p0").is_err());
        assert_eq!(parse(Field::Assignee, " me ").unwrap().assignee, Some(Some("me".into())));
        assert_eq!(parse(Field::Assignee, "").unwrap().assignee, Some(None));
        assert_eq!(parse(Field::Due, "+2d").unwrap().due, Some(Some(jiff::civil::date(2026, 10, 5))));
        assert!(parse(Field::Due, "someday").is_err());
        assert_eq!(parse(Field::Tags, "#a b a").unwrap().tags, Some(vec!["a".into(), "b".into()]));
        assert_eq!(parse(Field::Pr, "o/r#2").unwrap().prs.unwrap().len(), 2);
        assert!(parse(Field::Pr, "o/r#1").unwrap_err().contains("already linked"));
        assert!(parse(Field::Pr, "not a url").is_err());

        assert_eq!(Field::Priority.preview("u", &node, today).0, "→ Urgent");
        assert_eq!(Field::Due.preview("", &node, today).0, "→ None");
        assert_eq!(Field::Pr.preview("o/r#2", &node, today).0, "→ o/r#2");
        assert_eq!(Field::Due.preview("x", &node, today).0, "Not a date yet");
    }
}
