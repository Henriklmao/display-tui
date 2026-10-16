//! Test utilities for display-tui.

#[cfg(test)]
pub mod tests {
    use crate::monitor::{Monitor, Resolution, Position};
    use std::path::{Path, PathBuf};

    // A unique per-process temporary directory that removes itself on drop —
    // also when a test aborts on a failed assertion, so nothing is left
    // behind in /tmp.
    pub struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        // Creates a directory unique to this call, clearing any leftover
        // from an earlier run. The counter keeps parallel tests that pass
        // the same tag from clobbering each other's directory.
        pub fn new(tag: &str) -> Self {
            static NEXT_ID: std::sync::atomic::AtomicUsize =
                std::sync::atomic::AtomicUsize::new(0);
            let id = NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "display-tui-test-{}-{}-{}",
                std::process::id(),
                tag,
                id
            ));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();
            TempDir { path }
        }

        // The directory's path.
        pub fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    // Lets guards be used wherever a &Path is expected.
    impl std::ops::Deref for TempDir {
        type Target = Path;
        fn deref(&self) -> &Path {
            &self.path
        }
    }

    impl AsRef<Path> for TempDir {
        fn as_ref(&self) -> &Path {
            &self.path
        }
    }

    // Creates a standard set of test monitors.
    pub fn test_monitors() -> Vec<Monitor> {
        vec![
            Monitor {
                name: "Monitor 1".to_string(),
                description: Some("Description 1".to_string()),
                make: Some("Some".to_string()),
                model: Some("Description".to_string()),
                serial: Some("1".to_string()),
                enabled: true,
                modes: vec![
                    Resolution {
                        width: 1920,
                        height: 1080,
                        refresh: 60.0,
                        preferred: true,
                        current: true,
                    },
                    Resolution {
                        width: 1280,
                        height: 720,
                        refresh: 60.0,
                        preferred: false,
                        current: false,
                    },
                ],
                position: Some(Position { x: 0, y: 0 }),
                scale: Some(1.0),
                transform: None,
                saved_position: None,
                saved_scale: None,
                workspace: None,
            },
            Monitor {
                name: "Monitor 2".to_string(),
                description: Some("Description 2".to_string()),
                make: Some("Some".to_string()),
                model: Some("Description".to_string()),
                serial: Some("2".to_string()),
                enabled: false,
                modes: vec![
                    Resolution {
                        width: 1920,
                        height: 1080,
                        refresh: 60.0,
                        preferred: false,
                        current: false,
                    },
                    Resolution {
                        width: 1280,
                        height: 720,
                        refresh: 60.0,
                        preferred: true,
                        current: true,
                    },
                ],
                position: Some(Position { x: 1920, y: 0 }),
                scale: Some(1.25),
                transform: None,
                saved_position: None,
                saved_scale: None,
                workspace: None,
            },
        ]
    }
}
