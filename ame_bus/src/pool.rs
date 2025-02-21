use async_trait::async_trait;
use std::sync::Arc;
use tokio::task::JoinSet;

/// Concurrent pool for running async tasks. Based on tokio.
pub struct TokioPool {
    max_concurrent: usize,
    join_set: JoinSet<()>,
}

impl TokioPool {
    /// Create a new pool with the maximum number of concurrent tasks.
    pub fn new(max_concurrent: usize) -> Self {
        let join_set = JoinSet::new();
        Self {
            max_concurrent,
            join_set,
        }
    }

    /// run a task in the pool.
    ///
    /// The task must be a future that returns `()`.
    ///
    /// If the pool is full, the task will be blocked until a slot is available.
    pub async fn run<T>(&mut self, task: T)
    where
        T: std::future::Future<Output = ()> + Send + 'static,
    {
        if self.join_set.len() >= self.max_concurrent {
            self.join_set.join_next().await;
        }
        self.join_set.spawn(task);
    }
}

#[async_trait]
/// Trait for creating an async application with a pool.
pub trait PooledApp: Sized {

    /// The number of concurrent tasks per CPU core.
    const CPU_CONCURRENT: usize = 4;

    /// The maximum number of concurrent tasks.
    const MAX_CONCURRENT: usize = 100;

    /// Create a new [TokioPool] according to the configuration.
    fn create_pool() -> TokioPool {
        let logical_cpus = num_cpus::get();
        let max_current = std::cmp::min(Self::CPU_CONCURRENT * logical_cpus, Self::MAX_CONCURRENT);
        TokioPool::new(max_current)
    }

    /// Start the application with the pool. Must be implemented manually.
    ///
    /// Generally, you should create a future for each task, and run
    /// the task through the pool with [TokioPool::run]. But *it also work if you don't use the pool.*
    async fn start_with_pool(app: Arc<Self>, pool: TokioPool);

    /// internal function to start the application. Do not override.
    async fn start(self) {
        let pool = Self::create_pool();
        let app = Arc::new(self);
        Self::start_with_pool(app, pool).await;
    }
}
