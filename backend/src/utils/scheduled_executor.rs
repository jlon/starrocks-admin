// Scheduled Executor for periodic tasks
// Inspired by curvine's ScheduledExecutor
// Adapted for async/tokio runtime

use chrono::Utc;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

/// A trait for tasks that run periodically
pub trait ScheduledTask: Send + Sync + 'static {
    /// Execute the task
    /// Returns Ok(()) on success, Err on failure
    fn run(&self) -> Pin<Box<dyn Future<Output = Result<(), anyhow::Error>> + Send + '_>>;

    /// Check if the task should terminate
    /// Default: never terminate (run forever)
    fn should_terminate(&self) -> bool {
        false
    }

    /// Get task name for logging
    #[allow(dead_code)]
    fn name(&self) -> &str;
}

/// Scheduled executor for running periodic tasks
pub struct ScheduledExecutor {
    interval: Duration,
    task_name: String,
    shutdown: Arc<AtomicBool>,
}

impl ScheduledExecutor {
    /// Create a new scheduled executor
    ///
    /// # Arguments
    /// * `task_name` - Name of the task (for logging)
    /// * `interval` - Interval between executions
    pub fn new(task_name: impl Into<String>, interval: Duration) -> Self {
        Self {
            task_name: task_name.into(),
            interval,
            shutdown: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Get a handle to trigger shutdown
    pub fn shutdown_handle(&self) -> ScheduledExecutorHandle {
        ScheduledExecutorHandle {
            shutdown: self.shutdown.clone(),
        }
    }

    /// Start the scheduled task
    ///
    /// This spawns a tokio task that runs the provided task periodically.
    /// The task will continue running until:
    /// - `shutdown()` is called on the handle
    /// - The task's `should_terminate()` returns true
    ///
    /// # Example
    /// ```rust
    /// let executor = ScheduledExecutor::new("my-task", Duration::from_secs(30));
    /// let handle = executor.shutdown_handle();
    /// executor.start(my_task).await;
    /// 
    /// // Later, to stop:
    /// handle.shutdown();
    /// ```
    pub async fn start<T>(self, task: T)
    where
        T: ScheduledTask,
    {
        let task_name = self.task_name.clone();
        let interval_ms = self.interval.as_millis() as i64;
        let shutdown = self.shutdown;

        tracing::info!(
            "Starting scheduled task '{}' with interval: {:?}",
            task_name,
            self.interval
        );

        // Calculate next execution time
        let mut next_execution = Utc::now().timestamp_millis() + interval_ms;

        loop {
            // Check shutdown signals
            if shutdown.load(Ordering::Relaxed) || task.should_terminate() {
                tracing::info!("Scheduled task '{}' is shutting down", task_name);
                break;
            }

            let now = Utc::now().timestamp_millis();

            // Execute task if it's time
            if now >= next_execution {
                tracing::debug!("Executing scheduled task '{}'", task_name);

                match task.run().await {
                    Ok(()) => {
                        tracing::debug!("Scheduled task '{}' completed successfully", task_name);
                    }
                    Err(e) => {
                        // Log error but don't stop the scheduler
                        tracing::error!("Scheduled task '{}' failed: {}", task_name, e);
                    }
                }

                // Calculate next execution time (avoid cumulative drift)
                next_execution = Utc::now().timestamp_millis() + interval_ms;
            }

            // Calculate wait time until next execution
            let wait_ms = next_execution.saturating_sub(Utc::now().timestamp_millis());
            if wait_ms > 0 {
                sleep(Duration::from_millis(wait_ms as u64)).await;
            }
        }

        tracing::info!("Scheduled task '{}' stopped", task_name);
    }

    /// Start the scheduled task in a background tokio task
    ///
    /// This is a non-blocking version that spawns the task and returns immediately.
    ///
    /// # Returns
    /// A handle that can be used to shutdown the task
    pub fn spawn<T>(self, task: T) -> ScheduledExecutorHandle
    where
        T: ScheduledTask,
    {
        let handle = self.shutdown_handle();

        tokio::spawn(async move {
            self.start(task).await;
        });

        handle
    }
}

/// Handle to control a scheduled executor
#[derive(Clone)]
#[allow(dead_code)]
pub struct ScheduledExecutorHandle {
    shutdown: Arc<AtomicBool>,
}

#[allow(dead_code)]
impl ScheduledExecutorHandle {
    /// Signal the executor to shutdown
    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::Relaxed);
    }

    /// Check if shutdown has been requested
    pub fn is_shutdown(&self) -> bool {
        self.shutdown.load(Ordering::Relaxed)
    }
}

// =============================================================================
// Helper macros for implementing ScheduledTask
// =============================================================================

/// Macro to easily implement ScheduledTask for a type with async method
///
/// # Example
/// ```rust
/// struct MyTask {
///     db: SqlitePool,
/// }
///
/// impl MyTask {
///     async fn execute(&self) -> Result<(), anyhow::Error> {
///         // Your async logic here
///         Ok(())
///     }
/// }
///
/// impl_scheduled_task!(MyTask, "my-task", execute);
/// ```
#[macro_export]
macro_rules! impl_scheduled_task {
    ($type:ty, $name:expr, $method:ident) => {
        impl $crate::utils::ScheduledTask for $type {
            fn run(
                &self,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<(), anyhow::Error>> + Send + '_>,
            > {
                Box::pin(async move { self.$method().await })
            }

            fn name(&self) -> &str {
                $name
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU32;

    struct TestTask {
        counter: Arc<AtomicU32>,
        max_runs: u32,
    }

    impl ScheduledTask for TestTask {
        fn run(
            &self,
        ) -> Pin<Box<dyn Future<Output = Result<(), anyhow::Error>> + Send + '_>> {
            Box::pin(async move {
                let count = self.counter.fetch_add(1, Ordering::Relaxed);
                tracing::info!("TestTask run #{}", count + 1);
                Ok(())
            })
        }

        fn should_terminate(&self) -> bool {
            self.counter.load(Ordering::Relaxed) >= self.max_runs
        }

        fn name(&self) -> &str {
            "test-task"
        }
    }

    #[tokio::test]
    async fn test_scheduled_executor() {
        let counter = Arc::new(AtomicU32::new(0));
        let task = TestTask {
            counter: counter.clone(),
            max_runs: 3,
        };

        let executor = ScheduledExecutor::new("test", Duration::from_millis(100));
        executor.start(task).await;

        assert_eq!(counter.load(Ordering::Relaxed), 3);
    }

    #[tokio::test]
    async fn test_scheduled_executor_shutdown() {
        let counter = Arc::new(AtomicU32::new(0));
        let task = TestTask {
            counter: counter.clone(),
            max_runs: 1000, // Won't reach this
        };

        let executor = ScheduledExecutor::new("test-shutdown", Duration::from_millis(50));
        let handle = executor.shutdown_handle();

        // Spawn in background
        tokio::spawn(async move {
            executor.start(task).await;
        });

        // Let it run a few times
        tokio::time::sleep(Duration::from_millis(250)).await;

        // Shutdown
        handle.shutdown();

        // Wait for shutdown
        tokio::time::sleep(Duration::from_millis(100)).await;

        let final_count = counter.load(Ordering::Relaxed);
        assert!(final_count > 0 && final_count < 1000);
    }
}

