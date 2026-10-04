//! Command-line arguments of `topo-gui`.
//!
//! The parser is the same in every build, so a build without the `screenshot`
//! feature rejects `--screenshot` instead of mistaking it for a workspace path.

use std::path::PathBuf;

use clap::Parser;

/// The largest window side in points. The offscreen texture is twice that in
/// pixels, which stays well inside Metal's 32768-pixel limit.
const MAX_SIDE: i64 = 8192;

/// Desktop editor for a topo workspace
#[derive(Debug, Parser)]
#[command(name = "topo-gui", version)]
#[cfg_attr(not(feature = "screenshot"), allow(dead_code))]
pub struct Args {
    /// Directory to search for a `.topo` workspace (default: the current directory)
    pub workspace: Option<PathBuf>,

    /// Render one frame offscreen to a PNG instead of opening a window
    /// (needs a build with the `screenshot` feature)
    #[arg(long, value_name = "PATH")]
    pub screenshot: Option<PathBuf>,

    /// Captured window width in points
    #[arg(long, requires = "screenshot", value_name = "POINTS", default_value_t = 1360, value_parser = points())]
    pub width: u32,

    /// Captured window height in points
    #[arg(long, requires = "screenshot", value_name = "POINTS", default_value_t = 860, value_parser = points())]
    pub height: u32,

    /// Captured inspector width in points; derived from the window when
    /// omitted, never read from the saved preference, so a capture is reproducible
    #[arg(long, requires = "screenshot", value_name = "POINTS", value_parser = points())]
    pub inspector_width: Option<u32>,

    /// Ids or unique id prefixes of the nodes to select before capturing
    #[arg(long, requires = "screenshot", value_name = "ID", value_delimiter = ',', value_parser = node_id)]
    pub select: Vec<String>,

    /// Capture with this property of the one selected node open for editing
    #[arg(long, requires = "select", value_name = "FIELD")]
    pub edit: Option<crate::inline::Field>,

    /// Text typed into the field opened by `--edit`
    #[arg(long = "type", requires = "edit", value_name = "TEXT")]
    pub typed: Option<String>,

    /// Capture with the notes of the one selected node open for editing
    #[arg(long, requires = "select", conflicts_with = "edit")]
    pub edit_notes: bool,

    /// Replace the text of the notes opened by `--edit-notes`, which leaves them unsaved
    #[arg(long, requires = "edit_notes", value_name = "TEXT")]
    pub notes_text: Option<String>,

    /// Capture with the question about unsaved notes open (needs `--notes-text`)
    #[arg(long, requires = "notes_text")]
    pub unsaved_dialog: bool,

    /// Capture with the search prompt open on this query
    #[arg(long, requires = "screenshot", value_name = "QUERY")]
    pub search: Option<String>,

    /// Capture with the keyboard-shortcuts overlay open
    #[arg(long, requires = "screenshot")]
    pub help_overlay: bool,

    /// Capture with done and dropped nodes hidden; never read from the saved preference
    #[arg(long, requires = "screenshot")]
    pub hide_completed: bool,

    /// Capture with the nodes grouped by tag; never read from the saved preference
    #[arg(long, requires = "screenshot")]
    pub group_by_tag: bool,

    /// Tags (or `untagged`) whose groups are collapsed in the capture
    #[arg(long, requires = "group_by_tag", value_name = "TAG", value_delimiter = ',', value_parser = node_id)]
    pub collapse: Vec<String>,
}

/// A length in whole points that the renderer can allocate a texture for.
fn points() -> clap::builder::RangedI64ValueParser<u32> {
    clap::value_parser!(u32).range(1..=MAX_SIDE)
}

fn node_id(value: &str) -> Result<String, String> {
    match value.trim() {
        "" => Err("an id must not be empty".to_owned()),
        id => Ok(id.to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Args, clap::Error> {
        Args::try_parse_from(std::iter::once(&"topo-gui").chain(args))
    }

    #[test]
    fn a_lone_path_is_the_workspace() {
        assert_eq!(parse(&[]).unwrap().workspace, None);
        assert_eq!(parse(&["dir"]).unwrap().workspace, Some("dir".into()));
        assert!(parse(&["a", "b"]).is_err());
    }

    #[test]
    fn capture_options_are_parsed_in_any_order_and_value_form() {
        let args =
            parse(&["--width", "800", "--select", "a, b", "--select=c", "--screenshot=out.png", "--help-overlay"])
                .unwrap();
        assert_eq!(args.screenshot, Some("out.png".into()));
        assert_eq!((args.width, args.height), (800, 860));
        assert_eq!(args.select, ["a", "b", "c"]);
        assert!(args.help_overlay);
        // Without an override the inspector width is derived, not read from the user's config.
        assert_eq!(args.inspector_width, None);
    }

    #[test]
    fn options_are_rejected_rather_than_taken_for_a_workspace() {
        assert!(parse(&["--bogus"]).is_err());
        assert!(parse(&["--screenshot"]).is_err());
        // Capture options mean nothing without a capture.
        assert!(parse(&["--width", "800"]).is_err());
        assert!(parse(&["--help-overlay"]).is_err());
        assert!(parse(&["--screenshot", "o.png", "--select", "a,,b"]).is_err());
    }

    #[test]
    fn sizes_the_renderer_cannot_allocate_are_rejected() {
        for bad in ["0", "-5", "NaN", "800.5", "100000", "wide"] {
            for flag in ["--width", "--height", "--inspector-width"] {
                assert!(parse(&["--screenshot", "o.png", flag, bad]).is_err(), "{flag} {bad} must be rejected");
            }
        }
    }
}
