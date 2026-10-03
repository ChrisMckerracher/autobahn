//! What this machine has asked of the app, and where it is kept.
//!
//! Not in `config.toml`: that file is the fleet's, it is read by the
//! supervisor on every machine, and whether this window draws an icon
//! or raises a notification is nobody's business but this machine's.
//! So they live in `dash.toml`, beside it in the state root.

/// How much of itself the application shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Presence {
    /// A window and an item in the menu bar. What it has always done.
    #[default]
    Both,
    /// A window, and nothing in the menu bar.
    Window,
    /// An item in the menu bar, and no window until it is asked for —
    /// and on macOS, no icon in the dock either.
    Menubar,
}

impl Presence {
    pub fn parse(word: &str) -> Option<Presence> {
        match word.trim() {
            "both" => Some(Presence::Both),
            "window" => Some(Presence::Window),
            "menubar" => Some(Presence::Menubar),
            _ => None,
        }
    }

    pub fn word(self) -> &'static str {
        match self {
            Presence::Both => "both",
            Presence::Window => "window",
            Presence::Menubar => "menubar",
        }
    }

    pub fn opens_a_window(self) -> bool {
        !matches!(self, Presence::Menubar)
    }

    pub fn takes_the_menu_bar(self) -> bool {
        !matches!(self, Presence::Window)
    }
}

/// The file itself.
pub fn path(state_root: &std::path::Path) -> std::path::PathBuf {
    state_root.join("dash.toml")
}

/// What this machine has asked of the app.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    pub presence: Presence,
    /// Whether the app raises desktop notifications itself.
    ///
    /// On by default, and separate from `presence`, because the two are
    /// different questions: choosing a window over a menu bar item is
    /// about where the app appears, and it should not quietly decide
    /// whether anything tells you a session has halted.
    pub notify: bool,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            presence: Presence::default(),
            notify: true,
        }
    }
}

/// What the file says, falling back per key: a file that chose a
/// presence and says nothing about notifications has not turned them
/// off.
pub fn read(state_root: &std::path::Path) -> Settings {
    let Ok(text) = std::fs::read_to_string(path(state_root)) else {
        return Settings::default();
    };
    let Ok(table) = text.parse::<toml::Table>() else {
        return Settings::default();
    };
    Settings {
        presence: table
            .get("presence")
            .and_then(|value| value.as_str())
            .and_then(Presence::parse)
            .unwrap_or_default(),
        notify: table
            .get("notify")
            .and_then(|value| value.as_bool())
            .unwrap_or(true),
    }
}

/// Writes it back, and says why if it could not.
pub fn write(state_root: &std::path::Path, settings: Settings) -> Option<String> {
    let path = path(state_root);
    let text = format!(
        "# How much of itself the dash shows: both, window, menubar.\n\
         presence = \"{}\"\n\
         \n\
         # Whether the app raises desktop notifications itself. Leave this\n\
         # off if `on_alert` is set in the configuration: the hook follows\n\
         # the same rules, so both on means hearing everything twice.\n\
         notify = {}\n",
        settings.presence.word(),
        settings.notify,
    );
    std::fs::write(&path, text)
        .err()
        .map(|error| format!("{}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The word in the file and the word in the code are the same word,
    /// in both directions: a preference that did not survive a restart
    /// would be worse than no preference at all.
    #[test]
    fn a_presence_survives_being_written_down() {
        for presence in [Presence::Both, Presence::Window, Presence::Menubar] {
            assert_eq!(Presence::parse(presence.word()), Some(presence));
        }
        assert_eq!(Presence::parse("something else"), None);
        assert_eq!(Presence::default(), Presence::Both);
    }

    #[test]
    fn a_file_that_says_nothing_says_both() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        assert_eq!(read(directory.path()), Settings::default());

        let chosen = Settings {
            presence: Presence::Menubar,
            notify: false,
        };
        assert_eq!(write(directory.path(), chosen), None);
        assert_eq!(read(directory.path()), chosen);

        // A file somebody edited into nonsense is not a crash; it is a
        // file that has not chosen, which is what the default is for.
        std::fs::write(path(directory.path()), "presence = \"sideways\"\n").unwrap();
        assert_eq!(read(directory.path()), Settings::default());
        std::fs::write(path(directory.path()), "not toml [").unwrap();
        assert_eq!(read(directory.path()), Settings::default());
    }

    /// Each key falls back on its own. A file written before
    /// notifications were a choice chose a presence and nothing else,
    /// and must not read as having turned them off.
    #[test]
    fn a_file_that_names_one_key_keeps_the_default_for_the_other() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        std::fs::write(path(directory.path()), "presence = \"window\"\n").unwrap();
        assert_eq!(
            read(directory.path()),
            Settings {
                presence: Presence::Window,
                notify: true,
            }
        );
        std::fs::write(path(directory.path()), "notify = false\n").unwrap();
        assert_eq!(
            read(directory.path()),
            Settings {
                presence: Presence::Both,
                notify: false,
            }
        );
    }

    /// Which of the two things each mode asks for.
    #[test]
    fn each_mode_asks_for_what_it_is_named_after() {
        assert!(Presence::Both.opens_a_window() && Presence::Both.takes_the_menu_bar());
        assert!(Presence::Window.opens_a_window() && !Presence::Window.takes_the_menu_bar());
        assert!(!Presence::Menubar.opens_a_window() && Presence::Menubar.takes_the_menu_bar());
    }
}
