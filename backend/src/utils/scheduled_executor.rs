// Scheduled Executor for periodic tasks
// Inspired by curvine's ScheduledExecutor
// Adapted for async/tokio runtime

use chrono::Utc;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
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
}

/// Blanket implementation for Arc<T> where T: ScheduledTask
/// This allows passing Arc-wrapped tasks directly to the executor
impl<T: ScheduledTask> ScheduledTask for Arc<T> {
    fn run(&self) -> Pin<Box<dyn Future<Output = Result<(), anyhow::Error>> + Send + '_>> {
        (**self).run()
    }

    fn should_terminate(&self) -> bool {
        (**self).should_terminate()
    }
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
        Self { task_name: task_name.into(), interval, shutdown: Arc::new(AtomicBool::new(false)) }
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
                    },
                    Err(e) => {
                        // Log error but don't stop the scheduler
                        tracing::error!("Scheduled task '{}' failed: {}", task_name, e);
                    },
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
        fn run(&self) -> Pin<Box<dyn Future<Output = Result<(), anyhow::Error>> + Send + '_>> {
            Box::pin(async move {
                let count = self.counter.fetch_add(1, Ordering::Relaxed);
                tracing::info!("TestTask run #{}", count + 1);
                Ok(())
            })
        }

        fn should_terminate(&self) -> bool {
            self.counter.load(Ordering::Relaxed) >= self.max_runs
        }
    }

    #[tokio::test]
    async fn test_scheduled_executor() {
        let counter = Arc::new(AtomicU32::new(0));
        let task = TestTask { counter: counter.clone(), max_runs: 3 };

        let executor = ScheduledExecutor::new("test", Duration::from_millis(100));
        executor.start(task).await;

        assert_eq!(counter.load(Ordering::Relaxed), 3);
    }
}
