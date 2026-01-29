//! Server resource management and cleanup
//!
//! Handles graceful shutdown via signal handling and ensures cleanup
//! of temporary directories and resources via RAII Drop trait.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Manages server resources and handles graceful shutdown
#[derive(Debug)]
pub struct ServerResources {
    /// Base directory for temp files (FIFOs, etc.)
    base_dir: PathBuf,
    /// Atomic flag for quit signal
    quit_flag: Arc<AtomicBool>,
}

impl ServerResources {
    /// Create new ServerResources with the given base directory
    pub fn new(base_dir: PathBuf) -> Self {
        log::debug!(
            "Creating ServerResources with base_dir: {}",
            base_dir.display()
        );
        Self {
            base_dir,
            quit_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Setup signal handler for graceful shutdown
    ///
    /// Installs a SIGINT/SIGTERM handler that sets the quit flag
    pub fn setup_signal_handler(&self) -> Result<(), Box<dyn std::error::Error>> {
        let quit = self.quit_flag.clone();
        ctrlc::set_handler(move || {
            log::info!("SIGINT/SIGTERM received, initiating graceful shutdown");
            quit.store(true, Ordering::Relaxed);
        })?;
        log::debug!("Signal handler installed successfully");
        Ok(())
    }

    /// Check if quit signal has been received
    pub fn should_quit(&self) -> bool {
        self.quit_flag.load(Ordering::Relaxed)
    }

    /// Get the quit flag for sharing with threads
    pub fn quit_flag(&self) -> Arc<AtomicBool> {
        self.quit_flag.clone()
    }

    /// Get the base directory path
    pub fn base_dir(&self) -> &PathBuf {
        &self.base_dir
    }
}

impl Drop for ServerResources {
    fn drop(&mut self) {
        log::info!("Cleaning up server resources");

        // Remove temp directory and all contents (FIFOs, etc.)
        if self.base_dir.exists() {
            if let Err(e) = std::fs::remove_dir_all(&self.base_dir) {
                log::warn!(
                    "Failed to remove base_dir {}: {}",
                    self.base_dir.display(),
                    e
                );
            } else {
                log::debug!("Removed base_dir: {}", self.base_dir.display());
            }
        }

        log::info!("Server cleanup complete");
    }
}
