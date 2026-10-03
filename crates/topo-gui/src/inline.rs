//! Editing one property of the selected node in place, in its row of the inspector.
//!
//! Enter saves, Escape cancels, and so does a click elsewhere or anything else
//! that moves the focus: a half-typed value is never saved by accident. Input that cannot be
//! saved keeps the field open and says why under it. Tab and Shift-Tab save and
//! open the next or previous field.
//!
//! Under the field is a list of values to choose from, narrowed by what is
//! typed: the priorities, the assignees and tags the workspace already uses,
//! and common due dates. Down and Up move through it and Enter takes the
//! highlighted one. Tags are edited as chips: a space or comma finishes one,
//! and Backspace in the empty field removes the last.

use gpui::{App, Context, Entity, Focusable, Window};
use jiff::civil::Date;
use topo_core::model::{normalize_assignee, normalize_pr, pr_label};
use topo_core::{Edit, Graph, Node, NodeId, Priority};

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
    /// The text the field opened with. Until it is changed the list is not narrowed by it.
    initial: String,
    /// The finished tags of [`Field::Tags`]; the text field holds the one being typed.
    pub chips: Vec<String>,
    /// The entry of the list that Enter takes.
    pub highlight: Option<usize>,
}

/// A value offered under the field, and what tells it apart there.
pub(crate) struct Choice {
    pub value: String,
    pub detail: String,
}

/// Entries of the list shown at once.
const CHOICES: usize = 6;
/// The fields in the order Tab visits them.
const ORDER: [Field; 5] = [Field::Priority, Field::Assignee, Field::Due, Field::Tags, Field::Pr];

impl Field {
    fn placeholder(self) -> &'static str {
        match self {
            Field::Priority => "low, medium, high, urgent — empty clears",
            Field::Assignee => "Name or agent — empty clears",
            Field::Due => "2026-10-31, 10-31, +3d, tomorrow — empty clears",
            Field::Tags => "Add a tag…",
            Field::Pr => "URL or owner/repo#123",
        }
    }

    /// The text the field opens with: the current value.
    fn initial(self, node: &Node) -> String {
        match self {
            Field::Priority => node.priority.map(|p| p.to_string()).unwrap_or_default(),
            Field::Assignee => node.assignee.clone().unwrap_or_default(),
            Field::Due => node.due.map(|d| d.to_string()).unwrap_or_default(),
            Field::Tags | Field::Pr => String::new(),
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

impl Field {
    /// The values to offer for `typed`, best first. `chips` are the tags already in the field.
    fn choices(self, typed: &str, chips: &[String], graph: &Graph, today: Date) -> Vec<Choice> {
        let typed = typed.trim().trim_start_matches('#').to_lowercase();
        let choice = |value: String, detail: String| Choice { value, detail };
        let mut choices: Vec<Choice> = match self {
            Field::Priority => Priority::ALL
                .into_iter()
                .rev()
                .filter(|p| p.to_string().starts_with(&typed))
                .map(|p| choice(p.to_string(), theme::priority_text(p)))
                .collect(),
            Field::Assignee => {
                let mut names: Vec<&String> = graph.nodes().filter_map(|n| n.assignee.as_ref()).collect();
                names.sort();
                names.dedup();
                let matching = names.into_iter().filter(|name| name.to_lowercase().contains(&typed));
                matching.map(|name| choice(name.clone(), String::new())).collect()
            }
            Field::Due => ["today", "tomorrow", "+3d", "+1w", "+2w"]
                .into_iter()
                .filter(|preset| preset.starts_with(&typed))
                .filter_map(|preset| Some((preset, dates::parse_due(preset, today).ok()??)))
                .map(|(preset, date)| choice(preset.to_owned(), format!("{} {date}", date.strftime("%a"))))
                .collect(),
            Field::Tags => {
                let mut uses: Vec<(&String, usize)> = Vec::new();
                for tag in graph.nodes().flat_map(|n| &n.tags) {
                    match uses.iter_mut().find(|(t, _)| *t == tag) {
                        Some((_, count)) => *count += 1,
                        None => uses.push((tag, 1)),
                    }
                }
                // The most used tags first.
                uses.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
                let unused = uses.into_iter().filter(|(tag, _)| !chips.contains(tag));
                let matching = unused.filter(|(tag, _)| tag.to_lowercase().starts_with(&typed));
                matching.map(|(tag, count)| choice(tag.clone(), format!("{count}×"))).collect()
            }
            Field::Pr => Vec::new(),
        };
        choices.truncate(CHOICES);
        choices
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
        let chips = if field == Field::Tags { node.tags.clone() } else { Vec::new() };
        self.inline_input.update(cx, |input, cx| input.reset(&initial, field.placeholder(), cx));
        self.inline = Some(InlineEdit { node: id, field, error: None, initial, chips, highlight: None });
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

    /// What the field holds as one text: for tags, the chips and the tag being typed.
    pub(crate) fn inline_text(&self, cx: &App) -> String {
        let typed = self.inline_input.read(cx).text();
        match &self.inline {
            Some(edit) if edit.field == Field::Tags => format!("{} {typed}", edit.chips.join(" ")),
            _ => typed.to_owned(),
        }
    }

    /// The list under the field.
    pub(crate) fn inline_choices(&self, cx: &App) -> Vec<Choice> {
        let Some(edit) = &self.inline else { return Vec::new() };
        let text = self.inline_input.read(cx).text();
        // The value the field opened with is not a query.
        let typed = if text == edit.initial { "" } else { text };
        edit.field.choices(typed, &edit.chips, self.graph(), dates::today())
    }

    /// Saves the field and returns whether it did. A pull request field with nothing typed has nothing to save.
    fn save_inline(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(InlineEdit { node, field, .. }) = &self.inline else { return false };
        let (id, field) = (node.clone(), *field);
        let Some(node) = self.graph().get(&id) else { return true };
        let text = self.inline_text(cx);
        if field == Field::Pr && text.trim().is_empty() {
            return true;
        }
        match field.parse(&text, node, dates::today()) {
            Ok(edit) => self.mutate(cx, |graph| graph.edit(&id, edit)),
            Err(error) => {
                self.inline.as_mut().expect("checked above").error = Some(error);
                cx.notify();
                false
            }
        }
    }

    fn submit_inline(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.inline.as_ref().and_then(|edit| edit.highlight) {
            Some(index) => self.pick_choice(index, window, cx),
            None if self.save_inline(cx) => self.cancel_inline(window, cx),
            None => {}
        }
    }

    /// Takes entry `index` of the list: a tag joins the chips, any other value is saved.
    pub(crate) fn pick_choice(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(choice) = self.inline_choices(cx).into_iter().nth(index) else { return };
        let edit = self.inline.as_mut().expect("the list belongs to a field");
        edit.highlight = None;
        if edit.field == Field::Tags {
            edit.chips.push(choice.value);
            self.inline_input.update(cx, |input, cx| input.set_text("", cx));
            return cx.notify();
        }
        self.inline_input.update(cx, |input, cx| input.set_text(&choice.value, cx));
        if self.save_inline(cx) {
            self.cancel_inline(window, cx);
        }
    }

    /// Saves the field and opens the one after it, or before it, in the order of the panel.
    fn step_inline(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(field) = self.inline.as_ref().map(|edit| edit.field) else { return };
        if !self.save_inline(cx) {
            return;
        }
        let at = ORDER.iter().position(|f| *f == field).expect("every field is in the order");
        let next = ORDER[(at + if forward { 1 } else { ORDER.len() - 1 }) % ORDER.len()];
        self.start_inline(next, window, cx);
    }

    /// Moves finished tags from the text field to the chips: a tag is
    /// finished by the space or comma after it.
    fn collect_chips(&mut self, cx: &mut Context<Self>) {
        let Some(edit) = self.inline.as_mut().filter(|edit| edit.field == Field::Tags) else { return };
        let input = self.inline_input.read(cx);
        let text = input.text().to_owned();
        let separators = [',', ' ', '　'];
        // A composition may still turn its space into something else.
        if input.composing() || !text.contains(separators) {
            return;
        }
        let (finished, typing) = text.rsplit_once(separators).expect("the text has a separator");
        for tag in parse_tags(finished) {
            if !edit.chips.contains(&tag) {
                edit.chips.push(tag);
            }
        }
        let typing = typing.to_owned();
        self.inline_input.update(cx, |input, cx| input.set_text(&typing, cx));
    }

    pub(crate) fn remove_chip(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(edit) = &mut self.inline {
            edit.chips.remove(index);
            cx.notify();
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
                    edit.highlight = None;
                }
                self.collect_chips(cx);
            }
            InputEvent::Cancel => self.cancel_inline(window, cx),
            InputEvent::Submit => self.submit_inline(window, cx),
            InputEvent::Up | InputEvent::Down => {
                let last = self.inline_choices(cx).len().checked_sub(1);
                if let Some(edit) = &mut self.inline {
                    edit.highlight = match (event, edit.highlight, last) {
                        (_, _, None) => None,
                        (InputEvent::Down, None, _) => Some(0),
                        (InputEvent::Down, Some(i), Some(last)) => Some((i + 1).min(last)),
                        (_, Some(0) | None, _) => None,
                        (_, Some(i), _) => Some(i - 1),
                    };
                }
            }
            InputEvent::Next => self.step_inline(true, window, cx),
            InputEvent::Previous => self.step_inline(false, window, cx),
            InputEvent::BackspaceEmpty => {
                if let Some(edit) = &mut self.inline {
                    edit.chips.pop();
                }
            }
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

    #[test]
    fn choices_come_from_the_workspace_and_narrow_as_typed() {
        let today = jiff::civil::date(2026, 10, 3);
        let node = |id: &str, assignee: Option<&str>, tags: &[&str]| {
            let mut node = Node::new(NodeId(id.into()), Kind::Task, id.into());
            node.assignee = assignee.map(str::to_owned);
            node.tags = tags.iter().map(|t| (*t).to_owned()).collect();
            node
        };
        let graph = Graph::from_nodes([
            node("a", Some("claude"), &["gui", "core"]),
            node("b", Some("rai"), &["gui"]),
            node("c", Some("claude"), &["cloud"]),
        ])
        .unwrap();
        let values = |field: Field, typed: &str, chips: &[&str]| -> Vec<String> {
            let chips: Vec<String> = chips.iter().map(|c| (*c).to_owned()).collect();
            field.choices(typed, &chips, &graph, today).into_iter().map(|c| c.value).collect()
        };
        assert_eq!(values(Field::Priority, "", &[]), ["urgent", "high", "medium", "low"]);
        assert_eq!(values(Field::Priority, "M", &[]), ["medium"]);
        assert_eq!(values(Field::Assignee, "", &[]), ["claude", "rai"]);
        assert_eq!(values(Field::Assignee, "AI", &[]), ["rai"]);
        // The most used tag first; tags already in the field are not offered again.
        assert_eq!(values(Field::Tags, "", &[]), ["gui", "cloud", "core"]);
        assert_eq!(values(Field::Tags, "#c", &["core"]), ["cloud"]);
        assert_eq!(values(Field::Due, "+", &[]), ["+3d", "+1w", "+2w"]);
        assert_eq!(Field::Due.choices("tom", &[], &graph, today)[0].detail, "Sun 2026-10-04");
        assert!(values(Field::Pr, "", &[]).is_empty());
    }
}
