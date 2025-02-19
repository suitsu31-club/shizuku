use async_trait::async_trait;
use std::sync::Arc;
use tokio::task::JoinSet;

pub struct TokioPool {
    max_concurrent: usize,
    join_set: JoinSet<()>,
}

impl TokioPool {
    pub fn new(max_concurrent: usize) -> Self {
        let join_set = JoinSet::new();
        Self {
            max_concurrent,
            join_set,
        }
    }
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
pub trait PooledApp: Sized {
    type Config: Send + Sync;
    const CPU_CONCURRENT: usize = 4;
    const MAX_CONCURRENT: usize = 100;
    fn create_pool() -> TokioPool {
        let logical_cpus = num_cpus::get();
        let max_current = std::cmp::min(Self::CPU_CONCURRENT * logical_cpus, Self::MAX_CONCURRENT);
        TokioPool::new(max_current)
    }
    async fn start_with_pool(app: Arc<Self>, pool: TokioPool, config: Self::Config);
    async fn start(self, config: Self::Config) {
        let pool = Self::create_pool();
        let app = Arc::new(self);
        Self::start_with_pool(app, pool, config).await;
    }
}
