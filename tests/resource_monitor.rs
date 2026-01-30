//! Resource monitoring utilities for stress testing
//!
//! Provides functionality to monitor CPU and memory usage of processes
//! over time during test execution.

use std::time::{Duration, Instant};
use sysinfo::{get_current_pid, System};

/// A single resource usage sample
#[derive(Debug, Clone)]
pub struct ResourceSample {
    pub timestamp: Instant,
    pub elapsed_secs: f64,
    pub cpu_percent: f64,
    pub memory_mb: f64,
    pub virtual_memory_mb: f64,
}

/// Resource usage report with statistics
#[derive(Debug)]
pub struct ResourceReport {
    pub samples: Vec<ResourceSample>,
    pub duration_secs: f64,
    pub avg_cpu: f64,
    pub max_cpu: f64,
    pub avg_memory_mb: f64,
    pub max_memory_mb: f64,
    pub memory_growth_percent: f64,
    pub total_samples: usize,
}

impl ResourceReport {
    /// Print a formatted report to stdout
    pub fn print_report(&self) {
        println!("\n=== Resource Usage Report ===");
        println!("Duration: {:.1}s", self.duration_secs);
        println!("Samples: {}", self.total_samples);
        println!("Memory Average: {:.2} MB", self.avg_memory_mb);
        println!("Memory Max: {:.2} MB", self.max_memory_mb);
        println!("Memory Growth: {:.1}%", self.memory_growth_percent);
        println!("CPU Average: {:.1}%", self.avg_cpu);
        println!("CPU Max: {:.1}%", self.max_cpu);
        println!("===========================\n");
    }
}

/// Monitors resource usage of a process over time
pub struct ResourceMonitor {
    system: System,
    pid: sysinfo::Pid,
    start_time: Instant,
    samples: Vec<ResourceSample>,
    baseline_memory_mb: f64,
}

impl ResourceMonitor {
    /// Create a new monitor for the current process
    pub fn for_current_process() -> Self {
        Self::for_pid(get_current_pid().expect("failed to get current pid"))
    }

    /// Create a new monitor for a specific process
    pub fn for_pid(pid: sysinfo::Pid) -> Self {
        let mut system = System::new_all();
        system.refresh_all();

        // Get baseline memory
        let baseline_memory = system
            .process(pid)
            .map(|p| p.memory() as f64 / 1024.0 / 1024.0)
            .unwrap_or(0.0);

        Self {
            system,
            pid,
            start_time: Instant::now(),
            samples: Vec::new(),
            baseline_memory_mb: baseline_memory,
        }
    }

    /// Take a resource sample
    pub fn sample(&mut self) -> ResourceSample {
        self.system.refresh_process(self.pid);
        self.system.refresh_cpu();

        let elapsed = self.start_time.elapsed().as_secs_f64();

        let process = self.system.process(self.pid).expect("process disappeared");

        let sample = ResourceSample {
            timestamp: Instant::now(),
            elapsed_secs: elapsed,
            cpu_percent: process.cpu_usage() as f64,
            memory_mb: process.memory() as f64 / 1024.0 / 1024.0,
            virtual_memory_mb: process.virtual_memory() as f64 / 1024.0 / 1024.0,
        };

        self.samples.push(sample.clone());
        sample
    }

    /// Take samples at regular intervals for a duration
    pub fn sample_for(&mut self, duration: Duration, interval: Duration) -> Vec<ResourceSample> {
        let start = Instant::now();
        let mut samples = Vec::new();

        while start.elapsed() < duration {
            samples.push(self.sample());
            std::thread::sleep(interval);
        }

        samples
    }

    /// Take a sample if enough time has passed since the last one
    pub fn sample_if_elapsed(&mut self, min_interval: Duration) -> Option<ResourceSample> {
        if let Some(last) = self.samples.last() {
            if last.timestamp.elapsed() < min_interval {
                return None;
            }
        }
        Some(self.sample())
    }

    /// Generate a report from all collected samples
    pub fn report(&self) -> ResourceReport {
        if self.samples.is_empty() {
            return ResourceReport {
                samples: Vec::new(),
                duration_secs: 0.0,
                avg_cpu: 0.0,
                max_cpu: 0.0,
                avg_memory_mb: 0.0,
                max_memory_mb: 0.0,
                memory_growth_percent: 0.0,
                total_samples: 0,
            };
        }

        let duration_secs = self.start_time.elapsed().as_secs_f64();

        let avg_cpu =
            self.samples.iter().map(|s| s.cpu_percent).sum::<f64>() / self.samples.len() as f64;
        let max_cpu = self
            .samples
            .iter()
            .map(|s| s.cpu_percent)
            .fold(0.0, f64::max);

        let avg_memory_mb =
            self.samples.iter().map(|s| s.memory_mb).sum::<f64>() / self.samples.len() as f64;
        let max_memory_mb = self.samples.iter().map(|s| s.memory_mb).fold(0.0, f64::max);

        let memory_growth_percent = if self.baseline_memory_mb > 0.0 {
            ((max_memory_mb - self.baseline_memory_mb) / self.baseline_memory_mb) * 100.0
        } else {
            0.0
        };

        ResourceReport {
            samples: self.samples.clone(),
            duration_secs,
            avg_cpu,
            max_cpu,
            avg_memory_mb,
            max_memory_mb,
            memory_growth_percent,
            total_samples: self.samples.len(),
        }
    }

    /// Print a formatted report to stdout
    pub fn print_report(&self) {
        let report = self.report();
        println!("\n=== Resource Usage Report ===");
        println!("Duration: {:.1}s", report.duration_secs);
        println!("Samples: {}", report.total_samples);
        println!("Memory Baseline: {:.2} MB", self.baseline_memory_mb);
        println!("Memory Average: {:.2} MB", report.avg_memory_mb);
        println!("Memory Max: {:.2} MB", report.max_memory_mb);
        println!("Memory Growth: {:.1}%", report.memory_growth_percent);
        println!("CPU Average: {:.1}%", report.avg_cpu);
        println!("CPU Max: {:.1}%", report.max_cpu);
        println!("===========================\n");
    }
}

/// Multi-process resource monitor for monitoring both Kakoune and giallo-kak
pub struct MultiProcessMonitor {
    monitors: Vec<ResourceMonitor>,
    start_time: Instant,
}

impl MultiProcessMonitor {
    /// Create a monitor for multiple PIDs
    pub fn for_pids(pids: Vec<sysinfo::Pid>) -> Self {
        Self {
            monitors: pids.into_iter().map(ResourceMonitor::for_pid).collect(),
            start_time: Instant::now(),
        }
    }

    /// Sample all processes
    pub fn sample_all(&mut self) -> Vec<ResourceSample> {
        self.monitors.iter_mut().map(|m| m.sample()).collect()
    }

    /// Get combined report (sum of all processes)
    pub fn combined_report(&self) -> ResourceReport {
        let mut all_samples: Vec<ResourceSample> = Vec::new();

        // Merge samples by timestamp
        for monitor in &self.monitors {
            for sample in &monitor.samples {
                all_samples.push(sample.clone());
            }
        }

        // Sort by timestamp
        all_samples.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

        // Calculate combined stats
        if all_samples.is_empty() {
            return ResourceReport {
                samples: Vec::new(),
                duration_secs: 0.0,
                avg_cpu: 0.0,
                max_cpu: 0.0,
                avg_memory_mb: 0.0,
                max_memory_mb: 0.0,
                memory_growth_percent: 0.0,
                total_samples: 0,
            };
        }

        let duration_secs = self.start_time.elapsed().as_secs_f64();
        let avg_cpu =
            all_samples.iter().map(|s| s.cpu_percent).sum::<f64>() / all_samples.len() as f64;
        let max_cpu = all_samples
            .iter()
            .map(|s| s.cpu_percent)
            .fold(0.0, f64::max);
        let avg_memory_mb =
            all_samples.iter().map(|s| s.memory_mb).sum::<f64>() / all_samples.len() as f64;
        let max_memory_mb = all_samples.iter().map(|s| s.memory_mb).fold(0.0, f64::max);
        let total_samples = all_samples.len();

        ResourceReport {
            samples: all_samples,
            duration_secs,
            avg_cpu,
            max_cpu,
            avg_memory_mb,
            max_memory_mb,
            memory_growth_percent: 0.0, // Can't calculate for combined
            total_samples,
        }
    }
}
