use std::path;
use std::sync::mpsc::{self, TryIter};

use itertools::Itertools;
use notify::Watcher;

/// Stores configuration to track the file system the notes are stored in.
#[derive(Debug)]
pub struct FileTracker {
    /// Path to the vault to index.
    vault_path: path::PathBuf,
    /// File types to consider notes
    file_types: ignore::types::Types,
    /// Watcher that checks for file changes in the vault directory and needs to be kept alive with this index.
    /// Can be unused because it is just here for RAII.
    #[allow(unused)]
    watcher: notify::INotifyWatcher,
    /// Channel from which file change events in the vault directory are deposited by the watcher and can be requested.
    file_change_channel: mpsc::Receiver<Result<notify::Event, notify::Error>>,
}

impl FileTracker {
    pub fn new(config: &super::config::Config, vault_path: path::PathBuf) -> Self {
        // Pre-calculate allowed file types
        let mut types_builder = ignore::types::TypesBuilder::new();
        types_builder.add_defaults();
        for name in config.file_types.iter() {
            types_builder.select(name);
        }

        // Create asynchronous channel for file events.
        let (sender, receiver) = mpsc::channel();

        // Create watcher so we can store it in the file, delaying its drop (which stops its function) until the end of the lifetime of this index.
        let mut watcher = notify::recommended_watcher(move |res| {
            sender.send(res).unwrap();
        })
        .unwrap();

        // Start watching the vault.
        watcher
            .watch(&vault_path, notify::RecursiveMode::Recursive)
            .expect("Fixed config does not fail.");

        Self {
            vault_path,
            file_types: types_builder
                .build()
                .expect("To build predefined types correctly."),
            watcher,
            file_change_channel: receiver,
        }
    }
    /// Returns a file walker that iterates over all notes to index.
    pub fn get_walker(&self) -> ignore::Walk {
        ignore::WalkBuilder::new(&self.vault_path)
            .types(self.file_types.clone())
            .build()
    }

    /// Wether the given path is supposed to be tracked by rucola or not.
    /// Checks for file endings and gitignore
    pub fn is_tracked(&self, path: &path::PathBuf) -> bool {
        self.get_walker()
            .flatten()
            .map(|dir_entry| dir_entry.path().to_path_buf())
            .contains(path)
    }

    /// Returns an iterator over all events found by this tracker since the last check.
    pub fn try_events_iter(&self) -> TryIter<'_, Result<notify::Event, notify::Error>> {
        self.file_change_channel.try_iter()
    }
}
#[cfg(test)]
mod tests {
    use crate::files;

    #[test]
    fn test_file_endings() {
        let no_ending = std::path::PathBuf::from("./tests/common/notes/Booksold");
        let md = std::path::PathBuf::from("./tests/common/notes/Books.md");
        let txt = std::path::PathBuf::from("./tests/common/notes/Books.txt");
        let tex = std::path::PathBuf::from("./tests/common/notes/Books.tex");
        let md_ignored = std::path::PathBuf::from("./tests/.html/books.md");
        let html_ignored = std::path::PathBuf::from("./tests/.html/books.html");
        let md_foreign = std::path::PathBuf::from("./README.md");

        let config = files::Config::default();

        let tracker = super::FileTracker::new(&config, std::path::PathBuf::from("./tests/"));

        assert!(!tracker.is_tracked(&no_ending));
        assert!(tracker.is_tracked(&md));
        assert!(!tracker.is_tracked(&txt));
        assert!(!tracker.is_tracked(&tex));
        assert!(!tracker.is_tracked(&md_ignored));
        assert!(!tracker.is_tracked(&html_ignored));
        assert!(!tracker.is_tracked(&md_foreign));

        let tracker = super::FileTracker::new(
            &files::config::Config {
                file_types: vec!["md".to_owned(), "txt".to_owned()],
                ..Default::default()
            },
            std::path::PathBuf::from("./tests"),
        );

        assert!(!tracker.is_tracked(&no_ending));
        assert!(tracker.is_tracked(&md));
        assert!(tracker.is_tracked(&txt));
        assert!(!tracker.is_tracked(&tex));

        let tracker = super::FileTracker::new(
            &files::config::Config {
                file_types: vec!["all".to_owned()],
                ..Default::default()
            },
            std::path::PathBuf::from("./tests"),
        );

        assert!(!tracker.is_tracked(&no_ending));
        assert!(tracker.is_tracked(&md));
        assert!(tracker.is_tracked(&txt));
        assert!(tracker.is_tracked(&tex));
    }
}
