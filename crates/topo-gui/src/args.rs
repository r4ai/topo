//! Command-line arguments of `topo-gui`.
//!
//! The parser is the same in every build, so a build without the `screenshot`
//! feature rejects `--screenshot` instead of mistaking it for a workspace path.

use std::path::PathBuf;

use clap::{ArgGroup, Parser};

/// The largest window side in points. The offscreen texture is twice that in
/// pixels, which stays well inside Metal's 32768-pixel limit.
const MAX_SIDE: i64 = 8192;

/// Desktop editor for a topo workspace
#[derive(Debug, Parser)]
#[command(name = "topo-gui", version)]
#[command(group(ArgGroup::new("capture").args(["screenshot", "screenshot_all"]).multiple(false)))]
#[cfg_attr(not(feature = "screenshot"), allow(dead_code))]
pub struct Args {
    /// Directory to search for a `.topo` workspace (default: the current directory)
    pub workspace: Option<PathBuf>,

    /// Render one frame offscreen to a PNG instead of opening a window
    /// (needs a build with the `screenshot` feature)
    #[arg(long, value_name = "PATH")]
    pub screenshot: Option<PathBuf>,

    /// Render every QA state into this directory in one headless run, covering
    /// both themes and Monotone, and write an index.html to review them
    /// (needs a build with the `screenshot` feature)
    #[arg(long, value_name = "DIR", conflicts_with_all = ["workspace", "theme", "monotone"])]
    pub screenshot_all: Option<PathBuf>,

    /// Only render these states with `--screenshot-all` (repeatable or
    /// comma-separated; the default is every state named by `--list-states`)
    #[arg(long = "state", requires = "screenshot_all", value_name = "NAME", value_delimiter = ',')]
    pub states: Vec<String>,

    /// Print the QA state names `--screenshot-all` renders, then exit
    /// (needs a build with the `screenshot` feature)
    #[arg(long)]
    pub list_states: bool,

    /// Captured window width in points
    #[arg(long, requires = "capture", value_name = "POINTS", default_value_t = 1360, value_parser = points())]
    pub width: u32,

    /// Captured window height in points
    #[arg(long, requires = "capture", value_name = "POINTS", default_value_t = 860, value_parser = points())]
    pub height: u32,

    /// Captured inspector width in points; derived from the window when
    /// omitted, never read from the saved preference, so a capture is reproducible
    #[arg(long, requires = "capture", value_name = "POINTS", value_parser = points())]
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

    /// Capture with the theme popover (mode and Monotone switch) open
    #[arg(long, requires = "screenshot")]
    pub theme_menu: bool,

    /// Capture with the save-status overlay shown in this state
    #[arg(long, requires = "screenshot", value_enum, value_name = "STATE")]
    pub save_status: Option<CaptureSave>,

    /// Capture with an error toast showing this text
    #[arg(long, requires = "screenshot", value_name = "TEXT")]
    pub toast: Option<String>,

    /// Capture with done and dropped nodes hidden; never read from the saved preference
    #[arg(long, requires = "screenshot")]
    pub hide_completed: bool,

    /// Capture with the nodes grouped by tag; never read from the saved preference
    #[arg(long, requires = "screenshot")]
    pub group_by_tag: bool,

    /// Tags (or `untagged`) whose groups are collapsed in the capture
    #[arg(long, requires = "group_by_tag", value_name = "TAG", value_delimiter = ',', value_parser = node_id)]
    pub collapse: Vec<String>,

    /// Capture with the repository switcher open over the workspace. On a folder with no `.topo` it shows the
    /// switcher alone, as on the first launch
    #[arg(long, requires = "screenshot")]
    pub chooser: bool,

    /// A `.topo` directory to list as a recent workspace in the capture (repeatable); one that does not exist
    /// shows as missing. The saved history is never read
    #[arg(long, requires = "screenshot", value_name = "PATH")]
    pub recent: Vec<PathBuf>,

    /// Capture with this theme; never read from the saved preference
    #[arg(long, requires = "screenshot", value_enum, default_value_t = CaptureTheme::Dark)]
    pub theme: CaptureTheme,

    /// Capture with every signal hue drawn as a neutral; never read from the saved preference
    #[arg(long, requires = "screenshot")]
    pub monotone: bool,
}

/// The themes a capture can be rendered with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum CaptureTheme {
    Dark,
    Light,
}

/// The save-status states a capture can show.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum CaptureSave {
    /// In flight: "Saving changes…".
    Saving,
    /// Failed: "Changes need confirmation", with Retry and Discard.
    Error,
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
        assert!(parse(&["--theme-menu"]).is_err());
        assert!(parse(&["--screenshot", "o.png", "--select", "a,,b"]).is_err());
    }

    #[test]
    fn the_capture_theme_defaults_to_dark_and_needs_a_capture() {
        assert_eq!(parse(&["--screenshot", "o.png"]).unwrap().theme, CaptureTheme::Dark);
        assert_eq!(parse(&["--screenshot", "o.png", "--theme", "light"]).unwrap().theme, CaptureTheme::Light);
        assert!(parse(&["--screenshot", "o.png", "--theme", "bogus"]).is_err());
        assert!(parse(&["--theme", "light"]).is_err());
    }

    #[test]
    fn monotone_is_off_by_default_and_needs_a_capture() {
        assert!(!parse(&["--screenshot", "o.png"]).unwrap().monotone);
        assert!(parse(&["--screenshot", "o.png", "--monotone"]).unwrap().monotone);
        assert!(parse(&["--monotone"]).is_err());
    }

    #[test]
    fn the_save_status_capture_needs_a_capture_and_a_state() {
        let error = parse(&["--screenshot", "o.png", "--save-status", "error"]).unwrap();
        assert_eq!(error.save_status, Some(CaptureSave::Error));
        assert_eq!(
            parse(&["--screenshot", "o.png", "--save-status", "saving"]).unwrap().save_status,
            Some(CaptureSave::Saving)
        );
        assert!(parse(&["--save-status", "error"]).is_err());
        assert!(parse(&["--screenshot", "o.png", "--save-status", "bogus"]).is_err());
    }

    #[test]
    fn the_switcher_flags_need_a_capture_and_recents_repeat() {
        let args = parse(&["--screenshot", "o.png", "--chooser", "--recent", "a/.topo", "--recent=b/.topo"]).unwrap();
        assert!(args.chooser);
        assert_eq!(args.recent, [PathBuf::from("a/.topo"), PathBuf::from("b/.topo")]);
        assert!(!parse(&["--screenshot", "o.png"]).unwrap().chooser);
        assert!(parse(&["--chooser"]).is_err());
        assert!(parse(&["--recent", "a"]).is_err());
    }

    #[test]
    fn the_batch_capture_is_its_own_kind_of_capture() {
        let args =
            parse(&["--screenshot-all", "out", "--state", "default,help", "--state=search", "--width", "800"]).unwrap();
        assert_eq!(args.screenshot_all, Some("out".into()));
        assert_eq!(args.screenshot, None);
        assert_eq!(args.states, ["default", "help", "search"]);
        assert_eq!(args.width, 800);
        // The two capture modes are exclusive, and so are a workspace argument
        // and a theme/finish (the batch always covers them all).
        assert!(parse(&["--screenshot", "o.png", "--screenshot-all", "out"]).is_err());
        assert!(parse(&["--screenshot-all", "out", "dir"]).is_err());
        assert!(parse(&["--screenshot-all", "out", "--theme", "light"]).is_err());
        assert!(parse(&["--screenshot-all", "out", "--monotone"]).is_err());
        // `--state` and `--list-states` mean nothing without the batch capture.
        assert!(parse(&["--state", "default"]).is_err());
        assert_eq!(parse(&["--screenshot-all", "o"]).unwrap().states, Vec::<String>::new());
    }

    #[test]
    fn listing_the_states_stands_alone() {
        assert!(parse(&["--list-states"]).unwrap().list_states);
        assert!(!parse(&[]).unwrap().list_states);
        // It needs no capture, but it is still an option the parser accepts.
        assert!(parse(&["--list-states", "--width", "800"]).is_err());
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
