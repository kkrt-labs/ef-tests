use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Once;
use std::thread;
use std::time::{Duration, Instant};

use tracing_subscriber::{filter, FmtSubscriber};

static INIT: Once = Once::new();

pub fn setup() {
    INIT.call_once(|| {
        // Set-up tracing filter
        let filter = filter::EnvFilter::new("ef_testing=info,sequencer=info");
        let subscriber = FmtSubscriber::builder()
            .with_env_filter(filter)
            .without_time()
            .finish();
        tracing::subscriber::set_global_default(subscriber)
            .expect("setting tracing default failed");
    })
}

pub struct TestMonitor {
    /// thresholds in seconds
    thresholds: Vec<u64>,
}

impl TestMonitor {
    pub const fn new(thresholds: Vec<u64>) -> Self {
        Self { thresholds }
    }

    pub fn run<F, T>(&mut self, test_name: &str, test_fn: F) -> T
    where
        F: FnOnce() -> T,
    {
        let start_time = Instant::now();
        let is_running = Arc::new(AtomicBool::new(true));
        let is_running_clone = is_running.clone();

        // Spawn monitoring thread
        let test_name = test_name.to_string();
        let test_name_clone = test_name.clone();
        let thresholds = self.thresholds.clone();
        let monitor_thread = thread::spawn(move || {
            while is_running_clone.load(Ordering::SeqCst) {
                let duration = start_time.elapsed().as_secs();

                // Check each threshold
                for &threshold in &thresholds {
                    if duration > threshold {
                        println!(
                            "\nWARNING: Test '{}' has been running for over {} seconds\n\
                            Current duration: {:.1}s",
                            test_name, threshold, duration as f64
                        );
                        // Sleep longer for higher thresholds to reduce noise
                        thread::sleep(Duration::from_secs(std::cmp::min(threshold / 2, 30)));
                        break;
                    }
                }
                thread::sleep(Duration::from_secs(1));
            }
        });

        // Run the actual test
        let result = test_fn();

        // Stop the monitoring thread
        is_running.store(false, Ordering::SeqCst);
        let _ = monitor_thread.join();

        // Print final duration for slow tests
        let final_duration = start_time.elapsed().as_secs();
        if final_duration > self.thresholds[0] {
            println!(
                "\nTest '{}' completed in {:.1} seconds",
                test_name_clone, final_duration as f64
            );
        }

        result
    }
}

// Helper macro to make it easier to use
#[macro_export]
macro_rules! monitor_test {
    ($name:expr, $thresholds:expr, $test:expr) => {{
        let mut monitor = TestMonitor::new($thresholds.to_vec());
        monitor.run($name, $test)
    }};
}
