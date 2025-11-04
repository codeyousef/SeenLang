//! Work-stealing job system used by the Seen runtime.
//!
//! The job system exposes a deterministic `parallel_for` helper that runs on a
//! dedicated Rayon thread-pool sized to the host's parallelism.  For consumers
//! that cannot send mutable captures across threads (such as the interpreter)
//! a sequential fallback is also provided.

use rayon::{prelude::*, ThreadPool, ThreadPoolBuilder};
use std::cmp::max;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct JobSystem {
    worker_count: usize,
    pool: Arc<ThreadPool>,
}

impl JobSystem {
    /// Create a new job system backed by a Rayon thread-pool.
    pub fn new() -> Self {
        let workers = std::thread::available_parallelism()
            .map(|n| max(1, n.get()))
            .unwrap_or(1);

        let pool = ThreadPoolBuilder::new()
            .num_threads(workers)
            .thread_name(|idx| format!("seen-job-{}", idx))
            .build()
            .unwrap_or_else(|err| {
                // Fall back to a single-threaded pool if custom sizing fails.
                eprintln!("warning: falling back to single-threaded job system: {err}");
                ThreadPoolBuilder::new()
                    .num_threads(1)
                    .thread_name(|idx| format!("seen-job-fallback-{idx}"))
                    .build()
                    .expect("fallback job pool")
            });

        Self {
            worker_count: workers,
            pool: Arc::new(pool),
        }
    }

    /// Number of worker threads configured for the job system.
    pub fn worker_count(&self) -> usize {
        self.worker_count
    }

    /// Execute a job for each index in the range `[start, end)` using the job pool.
    ///
    /// The supplied closure must be thread-safe because it can be executed on
    /// multiple workers concurrently.
    pub fn parallel_for<F>(&self, start: usize, end: usize, job: F)
    where
        F: Fn(usize) + Send + Sync,
    {
        if start >= end {
            return;
        }

        self.pool.install(|| {
            let job_ref = &job;
            (start..end).into_par_iter().for_each(|idx| job_ref(idx));
        });
    }

    /// Sequential fallback for callers that rely on mutable captures.
    pub fn parallel_for_sequential<F>(&self, start: usize, end: usize, mut job: F)
    where
        F: FnMut(usize),
    {
        for idx in start..end {
            job(idx);
        }
    }
}

impl Default for JobSystem {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[test]
    fn parallel_for_visits_each_index() {
        let job_system = JobSystem::new();
        let len = 128;
        let flags: Vec<AtomicBool> = (0..len).map(|_| AtomicBool::new(false)).collect();
        let flags = Arc::new(flags);

        job_system.parallel_for(0, len, {
            let flags = Arc::clone(&flags);
            move |idx| {
                flags[idx].store(true, Ordering::SeqCst);
            }
        });

        assert!(
            flags.iter().all(|flag| flag.load(Ordering::SeqCst)),
            "expected every index to be visited exactly once"
        );
    }

    #[test]
    fn sequential_fallback_behaves_like_for_loop() {
        let job_system = JobSystem::new();
        let mut acc = 0usize;
        job_system.parallel_for_sequential(0, 5, |idx| acc += idx);
        assert_eq!(acc, 10);
    }
}
