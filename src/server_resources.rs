//! Server resource management and cleanup
//!
//! Handles graceful shutdown via signal handling and ensures cleanup
//! of temporary directories and resources via RAII Drop trait.

use std::path::PathBuf;
use std::process;
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
