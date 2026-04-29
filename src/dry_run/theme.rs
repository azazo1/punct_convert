use std::io::IsTerminal;

use colored::{ColoredString, Colorize};

#[derive(Clone, Copy)]
pub(super) struct DryRunTheme {
    enabled: bool,
}

impl DryRunTheme {
    pub(super) fn plain() -> Self {
        Self { enabled: false }
    }

    pub(super) fn colored_stdout() -> Self {
        Self {
            enabled: std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none(),
        }
    }

    pub(super) fn enabled(self) -> bool {
        self.enabled
    }

    fn paint(self, text: &str, f: impl FnOnce(&str) -> ColoredString) -> String {
        if self.enabled {
            f(text).to_string()
        } else {
            text.to_owned()
        }
    }

    pub(super) fn section_header(self, text: &str) -> String {
        self.paint(text, |s| s.bold().cyan())
    }

    pub(super) fn block_header(self, text: &str) -> String {
        self.paint(text, |s| s.bold().blue())
    }

    pub(super) fn summary(self, text: &str) -> String {
        self.paint(text, |s| s.bold().purple())
    }

    pub(super) fn status_will_convert(self, text: &str) -> String {
        self.paint(text, |s| s.bold().green())
    }

    pub(super) fn status_no_changes(self, text: &str) -> String {
        self.paint(text, |s| s.bold().yellow())
    }

    pub(super) fn status_unreadable(self, text: &str) -> String {
        self.paint(text, |s| s.bold().red())
    }

    pub(super) fn placeholder(self, text: &str) -> String {
        self.paint(text, |s| s.bright_black())
    }

    pub(super) fn visible_space(self) -> String {
        self.paint(" ", |s| s.on_bright_black())
    }
}
