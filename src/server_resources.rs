//! Server resource management and cleanup
//!
//! Handles graceful shutdown via signal handling, Kakoune session monitoring,
//! and ensures cleanup of temporary directories and resources via RAII Drop trait.

use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// Check if a Kakoune session is still running and responsive
pub fn is_kakoune_session_alive(session: &str) -> bool {
    let session = session.trim();
    if session.is_empty() {
        return true;
    }

    let candidate_paths = crate::kakoune::session_socket_paths(session);

    let mut any_candidate_found = false;
    for path in &candidate_paths {
        if path.exists() {
            any_candidate_found = true;
            if UnixStream::connect(path).is_ok() {
                return true;
            }
        }
    }

    // If socket file was found but connection was refused, session is dead
    if any_candidate_found {
        return false;
    }

    // Fallback: check `kak -l`
    if let Ok(output) = process::Command::new("kak").arg("-l").output() {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let trimmed = line.trim();
                if trimmed == session {
                    return true;
                }
            }
            return false;
        }
    }

    // If kak is not available and no socket was found, don't kill prematurely
    true
}

/// Manages server resources and handles graceful shutdown
#[derive(Debug)]
pub struct ServerResources {
    /// Base directory for temp files (FIFOs, etc.)
    base_dir: PathBuf,
    /// Atomic flag for quit signal
    quit_flag: Arc<AtomicBool>,
    /// Kakoune session name for lifecycle tracking
    session: Arc<Mutex<Option<String>>>,
}

impl ServerResources {
    /// Create new ServerResources with the given base directory and optional session name
    pub fn new(base_dir: PathBuf, session: Option<String>) -> Self {
        log::debug!(
            "Creating ServerResources with base_dir: {}, session: {:?}",
            base_dir.display(),
            session
        );
        Self {
            base_dir,
            quit_flag: Arc::new(AtomicBool::new(false)),
            session: Arc::new(Mutex::new(session)),
        }
    }

    /// Set or update the tracked Kakoune session name
    pub fn set_session(&self, session: String) {
        let mut s = self.session.lock().unwrap();
        if s.as_deref() != Some(&session) {
            log::debug!("Setting tracked Kakoune session to: {session}");
            *s = Some(session);
        }
    }

    /// Get current tracked session name
    #[allow(dead_code)]
    pub fn session(&self) -> Option<String> {
        self.session.lock().unwrap().clone()
    }

    /// Setup signal handler for graceful shutdown
    ///
    /// Installs a SIGINT/SIGTERM handler that sets the quit flag
    pub fn setup_signal_handler(&self) -> Result<(), Box<dyn std::error::Error>> {
        let quit = self.quit_flag.clone();
        let base_dir = self.base_dir.clone();
        ctrlc::set_handler(move || {
            log::info!("SIGINT/SIGTERM received, initiating graceful shutdown");
            quit.store(true, Ordering::Relaxed);
            cleanup_base_dir(&base_dir);
            process::exit(0);
        })?;
        log::debug!("Signal handler installed successfully");
        Ok(())
    }

    /// Start a background watcher that monitors the Kakoune session
    /// and terminates the server if the session exits
    pub fn start_session_watcher(&self) {
        let quit = self.quit_flag.clone();
        let base_dir = self.base_dir.clone();
        let session_lock = self.session.clone();

        thread::spawn(move || {
            log::debug!("Kakoune session watcher started");
            loop {
                if quit.load(Ordering::Relaxed) {
                    break;
                }

                thread::sleep(Duration::from_millis(1000));

                if quit.load(Ordering::Relaxed) {
                    break;
                }

                let current_session = {
                    let guard = session_lock.lock().unwrap();
                    guard.clone()
                };

                if let Some(ref s) = current_session {
                    if !is_kakoune_session_alive(s) {
                        // Double-check after 500ms to rule out transient hiccups
                        thread::sleep(Duration::from_millis(500));
                        if !is_kakoune_session_alive(s) {
                            log::info!(
                                "Kakoune session '{s}' is no longer running, shutting down server"
                            );
                            quit.store(true, Ordering::Relaxed);
                            cleanup_base_dir(&base_dir);
                            process::exit(0);
                        }
                    }
                }
            }
            log::debug!("Kakoune session watcher exiting");
        });
    }

    /// Check if quit signal has been received
    pub fn should_quit(&self) -> bool {
        self.quit_flag.load(Ordering::Relaxed)
    }

    /// Get the quit flag for sharing with threads
    pub fn quit_flag(&self) -> Arc<AtomicBool> {
        self.quit_flag.clone()
    }
}

fn cleanup_base_dir(base_dir: &PathBuf) {
    if base_dir.exists() {
        if let Err(e) = std::fs::remove_dir_all(base_dir) {
            log::warn!("Failed to remove base_dir {}: {}", base_dir.display(), e);
        } else {
            log::debug!("Removed base_dir: {}", base_dir.display());
        }
    }
}

impl Drop for ServerResources {
    fn drop(&mut self) {
        log::info!("Cleaning up server resources");

        // Remove temp directory and all contents (FIFOs, etc.)
        cleanup_base_dir(&self.base_dir);

        log::info!("Server cleanup complete");
    }
}
