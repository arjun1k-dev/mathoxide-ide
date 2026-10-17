use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};
use std::time::{Duration, Instant};

/// File watcher that notifies when the watched file changes on disk,
/// debouncing rapid duplicate events.
pub struct FileWatcher {
    path: PathBuf,
    rx: Receiver<notify::Result<Event>>,
    _watcher: RecommendedWatcher,
    last_event: Instant,
    debounce_duration: Duration,
}

impl FileWatcher {
    pub fn new(path: &Path) -> anyhow::Result<Self> {
        let (tx, rx) = channel();
        let mut watcher = RecommendedWatcher::new(
            move |res| {
                let _ = tx.send(res);
            },
            notify::Config::default(),
        )?;

        // Watch parent directory if path is a file, or path itself
        let watch_target = if path.is_file() {
            path.parent().unwrap_or(path)
        } else {
            path
        };

        watcher.watch(watch_target, RecursiveMode::NonRecursive)?;

        Ok(Self {
            path: path.to_path_buf(),
            rx,
            _watcher: watcher,
            last_event: Instant::now() - Duration::from_secs(1),
            debounce_duration: Duration::from_millis(100),
        })
    }

    /// Set a new file path to watch.
    pub fn watch_file(&mut self, path: &Path) -> anyhow::Result<()> {
        let watch_target = if path.is_file() {
            path.parent().unwrap_or(path)
        } else {
            path
        };
        self._watcher.watch(watch_target, RecursiveMode::NonRecursive)?;
        self.path = path.to_path_buf();
        Ok(())
    }

    /// Check if the watched file has changed since the last check.
    pub fn poll_changed(&mut self) -> bool {
        let mut changed = false;
        while let Ok(res) = self.rx.try_recv() {
            if let Ok(event) = res {
                // Check if any path in the event matches our watched file
                let matches_file = event.paths.iter().any(|p| {
                    p == &self.path
                        || p.file_name() == self.path.file_name()
                });

                if matches_file && (event.kind.is_modify() || event.kind.is_create()) {
                    let now = Instant::now();
                    if now.duration_since(self.last_event) > self.debounce_duration {
                        self.last_event = now;
                        changed = true;
                    }
                }
            }
        }
        changed
    }
}
