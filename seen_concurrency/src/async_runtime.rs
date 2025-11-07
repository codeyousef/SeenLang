//! Async runtime for Seen Language
//!
//! This module implements a cooperative async runtime with support for
//! Promise/Future types, async functions, and efficient task scheduling.
//! The runtime is designed for zero-cost abstractions with compile-time
//! optimization and runtime efficiency.

use crate::{
    channels::{ChannelManager, ManagerSelectFuture, SelectCase},
    types::{
        AsyncError, AsyncResult, AsyncValue, Channel, Promise, TaskHandle, TaskId, TaskPriority,
        TaskState,
    },
};
use once_cell::sync::Lazy;
use seen_lexer::position::Position;
use std::{
    cell::RefCell,
    collections::{HashMap, VecDeque},
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicU64, AtomicUsize, Ordering},
        Arc, Mutex, Weak,
    },
    task::{Context, Poll, Waker},
    time::{Duration, Instant},
};

const DEFAULT_STACK_FRAME_BYTES: usize = 2048;
const SCHEDULER_BACKOFF_THRESHOLD: u32 = 8;
const STARVATION_THRESHOLD: Duration = Duration::from_millis(5);
static NEXT_SCOPE_ID: AtomicU64 = AtomicU64::new(1);
static FALLBACK_FRAMES: Lazy<Mutex<Vec<CoroutineFrameData>>> = Lazy::new(|| Mutex::new(Vec::new()));

thread_local! {
    static ACTIVE_SCOPE_STACK: RefCell<Vec<Arc<ScopeArenaEntry>>> = RefCell::new(Vec::new());
}

/// Describes whether a coroutine frame may remain stack-bound within a scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EscapeClass {
    /// Frame lifetime is bounded by the current scope; resources can be stack-allocated/polled.
    ScopeBound,
    /// Frame may outlive the scope that created it; requires heap allocation.
    Detached,
}

/// Hints that guide the runtime when provisioning coroutine frames.
#[derive(Debug, Clone)]
pub struct CoroutineFrameHints {
    escape_class: EscapeClass,
    stack_budget_bytes: usize,
    scratch_hint_bytes: usize,
}

impl Default for CoroutineFrameHints {
    fn default() -> Self {
        Self::heap_only()
    }
}

impl CoroutineFrameHints {
    /// Prefer heap allocation for the coroutine frame.
    pub fn heap_only() -> Self {
        Self {
            escape_class: EscapeClass::Detached,
            stack_budget_bytes: DEFAULT_STACK_FRAME_BYTES,
            scratch_hint_bytes: DEFAULT_STACK_FRAME_BYTES,
        }
    }

    /// Prefer stack allocation scoped to the current task scope.
    pub fn scope_bound(stack_budget_bytes: usize) -> Self {
        Self {
            escape_class: EscapeClass::ScopeBound,
            stack_budget_bytes: stack_budget_bytes.max(DEFAULT_STACK_FRAME_BYTES),
            scratch_hint_bytes: stack_budget_bytes.max(DEFAULT_STACK_FRAME_BYTES),
        }
    }

    /// Desired stack budget for the frame (bytes).
    pub fn stack_budget_bytes(&self) -> usize {
        self.stack_budget_bytes
    }

    /// Hint for scratch buffer allocations.
    pub fn scratch_hint_bytes(&self) -> usize {
        self.scratch_hint_bytes
    }

    /// Escape classification derived from compiler analysis.
    pub fn escape_class(&self) -> EscapeClass {
        self.escape_class
    }

    /// Override the scratch buffer hint.
    pub fn with_scratch_hint(mut self, bytes: usize) -> Self {
        self.scratch_hint_bytes = bytes.max(64);
        self
    }
}

/// Options used when spawning a task, including priority and frame hints.
#[derive(Debug, Clone)]
pub struct TaskSpawnOptions {
    pub priority: TaskPriority,
    pub frame_hints: Option<CoroutineFrameHints>,
}

impl TaskSpawnOptions {
    /// Create spawn options with default heap-based frame allocation.
    pub fn new(priority: TaskPriority) -> Self {
        Self {
            priority,
            frame_hints: None,
        }
    }

    /// Specify explicit frame hints.
    pub fn with_frame_hints(mut self, hints: CoroutineFrameHints) -> Self {
        self.frame_hints = Some(hints);
        self
    }

    /// Helper for scope-bound stack allocations.
    pub fn scope_bound(priority: TaskPriority, stack_budget_bytes: usize) -> Self {
        Self {
            priority,
            frame_hints: Some(CoroutineFrameHints::scope_bound(stack_budget_bytes)),
        }
    }
}

#[derive(Debug)]
struct FrameContextParts {
    local_state: HashMap<String, AsyncValue>,
    stack_trace: Vec<String>,
    scratch: Vec<u8>,
}

impl Default for FrameContextParts {
    fn default() -> Self {
        Self {
            local_state: HashMap::new(),
            stack_trace: Vec::new(),
            scratch: Vec::with_capacity(DEFAULT_STACK_FRAME_BYTES),
        }
    }
}

#[derive(Debug)]
struct CoroutineFrameData {
    scope: Option<Weak<ScopeArenaEntry>>,
    hints: CoroutineFrameHints,
    local_state: HashMap<String, AsyncValue>,
    stack_trace: Vec<String>,
    scratch: Vec<u8>,
}

impl CoroutineFrameData {
    fn new_heap(hints: CoroutineFrameHints) -> Self {
        Self::new_with_scope(None, hints)
    }

    fn new_with_scope(scope: Option<Weak<ScopeArenaEntry>>, hints: CoroutineFrameHints) -> Self {
        let mut data = Self {
            scope,
            hints: hints.clone(),
            local_state: HashMap::new(),
            stack_trace: Vec::new(),
            scratch: Vec::with_capacity(hints.stack_budget_bytes()),
        };
        data.ensure_scratch_capacity(hints.scratch_hint_bytes());
        data
    }

    fn checkout(&mut self) -> FrameContextParts {
        FrameContextParts {
            local_state: std::mem::take(&mut self.local_state),
            stack_trace: std::mem::take(&mut self.stack_trace),
            scratch: std::mem::take(&mut self.scratch),
        }
    }

    fn restore(&mut self, mut parts: FrameContextParts) {
        parts.local_state.clear();
        parts.stack_trace.clear();
        parts.scratch.clear();
        self.local_state = parts.local_state;
        self.stack_trace = parts.stack_trace;
        self.scratch = parts.scratch;
    }

    fn retarget(&mut self, scope: Option<Weak<ScopeArenaEntry>>, hints: &CoroutineFrameHints) {
        self.scope = scope;
        self.hints = hints.clone();
        self.ensure_scratch_capacity(hints.scratch_hint_bytes());
    }

    fn ensure_scratch_capacity(&mut self, desired: usize) {
        if self.scratch.capacity() < desired {
            self.scratch.reserve(desired - self.scratch.capacity());
        }
    }

    fn reset_contents(&mut self) {
        self.local_state.clear();
        self.stack_trace.clear();
        self.scratch.clear();
    }

    fn into_heap(mut self) -> Self {
        self.scope = None;
        self
    }
}

#[derive(Debug)]
struct ScopeArenaEntry {
    id: u64,
    kind: TaskScopeKind,
    stack_budget_bytes: usize,
    recycled: Mutex<Vec<CoroutineFrameData>>,
    outstanding: AtomicUsize,
}

impl ScopeArenaEntry {
    fn new(kind: TaskScopeKind, stack_budget_bytes: usize) -> Self {
        Self {
            id: NEXT_SCOPE_ID.fetch_add(1, Ordering::SeqCst),
            kind,
            stack_budget_bytes: stack_budget_bytes.max(DEFAULT_STACK_FRAME_BYTES),
            recycled: Mutex::new(Vec::new()),
            outstanding: AtomicUsize::new(0),
        }
    }

    fn checkout_frame(self: &Arc<Self>, hints: &CoroutineFrameHints) -> (CoroutineFrameData, bool) {
        self.outstanding.fetch_add(1, Ordering::SeqCst);
        let mut recycled = self.recycled.lock().expect("scope recycled mutex poisoned");
        if let Some(mut frame) = recycled.pop() {
            frame.reset_contents();
            frame.retarget(Some(Arc::downgrade(self)), hints);
            return (frame, true);
        }
        (
            CoroutineFrameData::new_with_scope(Some(Arc::downgrade(self)), hints.clone()),
            false,
        )
    }

    fn recycle_frame(self: &Arc<Self>, mut frame: CoroutineFrameData) {
        let hints = frame.hints.clone();
        frame.reset_contents();
        frame.retarget(Some(Arc::downgrade(self)), &hints);
        let mut recycled = self.recycled.lock().expect("scope recycled mutex poisoned");
        recycled.push(frame);
        self.outstanding.fetch_sub(1, Ordering::SeqCst);
    }

    fn drain_recycled(&self) -> Vec<CoroutineFrameData> {
        let mut recycled = self.recycled.lock().expect("scope recycled mutex poisoned");
        recycled.drain(..).collect()
    }
}

struct ScopeStack;

impl ScopeStack {
    fn push(kind: TaskScopeKind, stack_budget_bytes: usize) -> Arc<ScopeArenaEntry> {
        let entry = Arc::new(ScopeArenaEntry::new(kind, stack_budget_bytes));
        ACTIVE_SCOPE_STACK.with(|stack| stack.borrow_mut().push(Arc::clone(&entry)));
        entry
    }

    fn pop(expected_id: u64) -> Option<Arc<ScopeArenaEntry>> {
        ACTIVE_SCOPE_STACK.with(|stack| {
            let mut stack = stack.borrow_mut();
            let entry = stack.pop()?;
            debug_assert_eq!(
                entry.id, expected_id,
                "scope guard drop order violated (expected {}, saw {})",
                expected_id, entry.id
            );
            Some(entry)
        })
    }

    fn with_current<F, R>(f: F) -> Option<R>
    where
        F: FnOnce(&Arc<ScopeArenaEntry>) -> R,
    {
        ACTIVE_SCOPE_STACK
            .with(|stack| stack.borrow().last().cloned())
            .map(|entry| f(&entry))
    }

    fn recycle_frame(frame: CoroutineFrameData) {
        if let Some(scope) = frame.scope.as_ref().and_then(|weak| weak.upgrade()) {
            scope.recycle_frame(frame);
        } else {
            FALLBACK_FRAMES
                .lock()
                .expect("fallback frame pool poisoned")
                .push(frame.into_heap());
        }
    }

    fn release_entry(entry: Arc<ScopeArenaEntry>) {
        let outstanding = entry.outstanding.load(Ordering::SeqCst);
        if outstanding != 0 {
            eprintln!(
                "seen_concurrency: scope {} dropped with {} outstanding coroutine frames",
                entry.id, outstanding
            );
        }
        let mut fallback = FALLBACK_FRAMES
            .lock()
            .expect("fallback frame pool poisoned");
        for frame in entry.drain_recycled() {
            fallback.push(frame.into_heap());
        }
    }
}

fn checkout_heap_frame(hints: &CoroutineFrameHints) -> (CoroutineFrameData, bool) {
    let mut fallback = FALLBACK_FRAMES
        .lock()
        .expect("fallback frame pool poisoned");
    if let Some(mut frame) = fallback.pop() {
        frame.reset_contents();
        frame.retarget(None, hints);
        return (frame, true);
    }
    (CoroutineFrameData::new_heap(hints.clone()), false)
}

/// Scope classification for structured concurrency.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskScopeKind {
    Async,
    Jobs,
}

/// Guard returned when entering a scoped task arena. Releases resources on drop.
#[derive(Debug)]
pub struct TaskScopeGuard {
    entry: Option<Arc<ScopeArenaEntry>>,
}

impl TaskScopeGuard {
    fn new(kind: TaskScopeKind, stack_budget_bytes: usize) -> Self {
        let entry = ScopeStack::push(kind, stack_budget_bytes);
        Self { entry: Some(entry) }
    }
}

impl Drop for TaskScopeGuard {
    fn drop(&mut self) {
        if let Some(entry) = self.entry.take() {
            if let Some(popped) = ScopeStack::pop(entry.id) {
                ScopeStack::release_entry(popped);
            }
        }
    }
}

#[derive(Debug, Default)]
struct RuntimeMetricsInner {
    stack_frames_allocated: AtomicU64,
    stack_frames_reused: AtomicU64,
    stack_frame_fallbacks: AtomicU64,
    heap_frames_allocated: AtomicU64,
}

impl RuntimeMetricsInner {
    fn record_stack_allocation(&self, reused: bool) {
        self.stack_frames_allocated.fetch_add(1, Ordering::SeqCst);
        if reused {
            self.stack_frames_reused.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn record_stack_fallback(&self) {
        self.stack_frame_fallbacks.fetch_add(1, Ordering::SeqCst);
    }

    fn record_heap_allocation(&self) {
        self.heap_frames_allocated.fetch_add(1, Ordering::SeqCst);
    }

    fn snapshot(&self) -> FrameMetricsSnapshot {
        FrameMetricsSnapshot {
            stack_frames_allocated: self.stack_frames_allocated.load(Ordering::SeqCst),
            stack_frames_reused: self.stack_frames_reused.load(Ordering::SeqCst),
            stack_frame_fallbacks: self.stack_frame_fallbacks.load(Ordering::SeqCst),
            heap_frames_allocated: self.heap_frames_allocated.load(Ordering::SeqCst),
        }
    }
}

/// Frame-level metrics snapshot.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FrameMetricsSnapshot {
    pub stack_frames_allocated: u64,
    pub stack_frames_reused: u64,
    pub stack_frame_fallbacks: u64,
    pub heap_frames_allocated: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SchedulerMetricsSnapshot {
    pub scheduled_high: u64,
    pub scheduled_normal: u64,
    pub scheduled_low: u64,
    pub dispatched_high: u64,
    pub dispatched_normal: u64,
    pub dispatched_low: u64,
    pub wake_promotions: u64,
    pub idle_backoffs: u64,
    pub idle_polls: u64,
    pub wake_latency_ns_total: u128,
    pub wake_latency_samples: u64,
    pub wake_latency_max_ns: u128,
    pub starvation_events: u64,
}

#[derive(Debug, Default, Clone)]
struct SchedulerMetrics {
    scheduled_high: u64,
    scheduled_normal: u64,
    scheduled_low: u64,
    dispatched_high: u64,
    dispatched_normal: u64,
    dispatched_low: u64,
    wake_promotions: u64,
    idle_backoffs: u64,
    idle_polls: u64,
    wake_latency_ns_total: u128,
    wake_latency_samples: u64,
    wake_latency_max_ns: u128,
    starvation_events: u64,
}

impl SchedulerMetrics {
    fn snapshot(&self) -> SchedulerMetricsSnapshot {
        SchedulerMetricsSnapshot {
            scheduled_high: self.scheduled_high,
            scheduled_normal: self.scheduled_normal,
            scheduled_low: self.scheduled_low,
            dispatched_high: self.dispatched_high,
            dispatched_normal: self.dispatched_normal,
            dispatched_low: self.dispatched_low,
            wake_promotions: self.wake_promotions,
            idle_backoffs: self.idle_backoffs,
            idle_polls: self.idle_polls,
            wake_latency_ns_total: self.wake_latency_ns_total,
            wake_latency_samples: self.wake_latency_samples,
            wake_latency_max_ns: self.wake_latency_max_ns,
            starvation_events: self.starvation_events,
        }
    }
}

/// Human-readable runtime metrics snapshot.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeMetricsSnapshot {
    pub stack_frames_allocated: u64,
    pub stack_frames_reused: u64,
    pub stack_frame_fallbacks: u64,
    pub heap_frames_allocated: u64,
    pub scheduler: SchedulerMetricsSnapshot,
}

/// Main async runtime for executing Seen async operations
#[derive(Debug)]
pub struct AsyncRuntime {
    /// Task scheduler for managing async tasks
    scheduler: TaskScheduler,
    /// Task registry for tracking all running tasks
    task_registry: TaskRegistry,
    /// Promise resolver for handling Promise completion
    promise_resolver: PromiseResolver,
    /// Configuration options
    config: AsyncRuntimeConfig,
    /// Pending wake signals generated by wakers
    wake_queue: Arc<Mutex<Vec<TaskId>>>,
    /// Runtime metrics shared across operations
    metrics: Arc<RuntimeMetricsInner>,
}

/// Configuration for async runtime behavior
#[derive(Debug, Clone)]
pub struct AsyncRuntimeConfig {
    /// Maximum number of concurrent tasks
    pub max_concurrent_tasks: usize,
    /// Enable task priority scheduling
    pub enable_priority_scheduling: bool,
    /// Maximum task execution time before yielding (microseconds)
    pub max_task_execution_time_us: u64,
    /// Enable async stack trace collection
    pub enable_async_stack_traces: bool,
    /// Task cleanup threshold
    pub task_cleanup_threshold: usize,
}

impl Default for AsyncRuntimeConfig {
    fn default() -> Self {
        Self {
            max_concurrent_tasks: 1000,
            enable_priority_scheduling: true,
            max_task_execution_time_us: 1000, // 1ms
            enable_async_stack_traces: true,
            task_cleanup_threshold: 100,
        }
    }
}

/// Task scheduler for managing async task execution
#[derive(Debug)]
pub struct TaskScheduler {
    /// Ready task queue
    ready_queue: VecDeque<(TaskPriority, TaskId)>,
    /// Priority task queue (if priority scheduling enabled)
    priority_queue: VecDeque<(TaskPriority, TaskId)>,
    /// Waiting tasks (blocked on I/O, etc.)
    waiting_tasks: HashMap<TaskId, TaskState>,
    /// Currently executing task
    current_task: Option<TaskId>,
    /// Scheduler instrumentation
    metrics: SchedulerMetrics,
    /// Idle polls since last dispatched task (for backoff)
    idle_polls_since_task: u32,
    /// Timestamp of when tasks entered the ready queues
    queued_at: HashMap<TaskId, Instant>,
}

/// Entry inside the task registry arena
#[derive(Debug)]
struct TaskSlot {
    generation: u32,
    task: Option<AsyncTask>,
    result: Option<AsyncResult>,
}

impl TaskSlot {
    fn new() -> Self {
        Self {
            generation: 0,
            task: None,
            result: None,
        }
    }
}

/// Registry for tracking all tasks in the system
#[derive(Debug)]
pub struct TaskRegistry {
    slots: Vec<TaskSlot>,
    free_list: Vec<u32>,
}

/// Promise resolver for handling Promise completion and chaining
#[derive(Debug)]
pub struct PromiseResolver {
    /// Pending promises waiting for resolution
    pending_promises: HashMap<TaskId, Promise>,
    /// Promise chains for .then() operations
    promise_chains: HashMap<TaskId, Vec<TaskId>>,
    /// Resolved promise values
    resolved_values: HashMap<TaskId, AsyncValue>,
}

/// Represents an async task in the runtime
#[derive(Debug)]
pub struct AsyncTask {
    /// Unique task identifier
    pub id: TaskId,
    /// Task execution state
    pub state: TaskState,
    /// Task priority level
    pub priority: TaskPriority,
    /// Async function to execute
    pub function: Box<dyn AsyncFunction>,
    /// Position where task was created
    pub created_at: Position,
    /// Task dependencies
    pub dependencies: Vec<TaskId>,
    /// Waker for task resumption
    pub waker: Option<Waker>,
    /// Backing coroutine frame for reuse/recycling
    frame: CoroutineFrameData,
    /// Original hints used to size the frame (for recycling defaults)
    frame_hints: CoroutineFrameHints,
}

#[derive(Debug)]
struct TaskWakeHandle {
    task_id: TaskId,
    wake_queue: Arc<Mutex<Vec<TaskId>>>,
}

impl TaskWakeHandle {
    fn new(task_id: TaskId, wake_queue: Arc<Mutex<Vec<TaskId>>>) -> Self {
        Self {
            task_id,
            wake_queue,
        }
    }

    fn wake(&self) {
        if let Ok(mut queue) = self.wake_queue.lock() {
            queue.push(self.task_id);
        }
    }
}

/// Trait for async function execution
pub trait AsyncFunction: Send + Sync + std::fmt::Debug {
    /// Execute the async function
    fn execute(
        &self,
        context: &mut AsyncExecutionContext,
    ) -> Pin<Box<dyn Future<Output = AsyncResult> + Send>>;

    /// Get function name for debugging
    fn name(&self) -> &str;

    /// Check if function is pure (no side effects)
    fn is_pure(&self) -> bool {
        false
    }

    /// Frame allocation hints emitted by the compiler/runtime analysis.
    fn frame_hints(&self) -> CoroutineFrameHints {
        CoroutineFrameHints::default()
    }
}

/// Context for async function execution
#[derive(Debug)]
pub struct AsyncExecutionContext {
    /// Current task ID
    pub task_id: TaskId,
    /// Runtime reference for spawning subtasks
    pub runtime: Arc<Mutex<AsyncRuntime>>,
    /// Local variables and state
    pub local_state: HashMap<String, AsyncValue>,
    /// Execution stack trace
    pub stack_trace: Vec<String>,
    /// Scratch buffer backed by the coroutine frame allocation
    scratch_pad: Vec<u8>,
}

impl AsyncExecutionContext {
    fn from_parts(
        task_id: TaskId,
        runtime: Arc<Mutex<AsyncRuntime>>,
        parts: FrameContextParts,
    ) -> Self {
        Self {
            task_id,
            runtime,
            local_state: parts.local_state,
            stack_trace: parts.stack_trace,
            scratch_pad: parts.scratch,
        }
    }

    fn into_parts(self) -> FrameContextParts {
        FrameContextParts {
            local_state: self.local_state,
            stack_trace: self.stack_trace,
            scratch: self.scratch_pad,
        }
    }

    /// Mutable scratch buffer callers can use for temporary allocations without hitting the heap.
    pub fn scratch_mut(&mut self) -> &mut Vec<u8> {
        &mut self.scratch_pad
    }
}

impl AsyncRuntime {
    /// Create a new async runtime with default configuration
    pub fn new() -> Self {
        Self {
            scheduler: TaskScheduler::new(),
            task_registry: TaskRegistry::new(),
            promise_resolver: PromiseResolver::new(),
            config: AsyncRuntimeConfig::default(),
            wake_queue: Arc::new(Mutex::new(Vec::new())),
            metrics: Arc::new(RuntimeMetricsInner::default()),
        }
    }

    /// Create async runtime with custom configuration
    pub fn with_config(config: AsyncRuntimeConfig) -> Self {
        Self {
            scheduler: TaskScheduler::new(),
            task_registry: TaskRegistry::new(),
            promise_resolver: PromiseResolver::new(),
            config,
            wake_queue: Arc::new(Mutex::new(Vec::new())),
            metrics: Arc::new(RuntimeMetricsInner::default()),
        }
    }

    /// Spawn a new async task
    pub fn spawn_task(
        &mut self,
        function: Box<dyn AsyncFunction>,
        priority: TaskPriority,
    ) -> TaskHandle {
        self.spawn_task_with_options(
            function,
            TaskSpawnOptions {
                priority,
                frame_hints: None,
            },
        )
    }

    /// Spawn a task with explicit frame hints/options.
    pub fn spawn_task_with_options(
        &mut self,
        mut function: Box<dyn AsyncFunction>,
        options: TaskSpawnOptions,
    ) -> TaskHandle {
        // Check task limits
        if self.task_registry.active_task_count() >= self.config.max_concurrent_tasks {
            return TaskHandle::error(AsyncError::TaskLimitExceeded {
                limit: self.config.max_concurrent_tasks,
                position: Position::new(0, 0, 0),
            });
        }

        let hints = options
            .frame_hints
            .clone()
            .unwrap_or_else(|| function.frame_hints());
        let frame = self.allocate_frame(&hints);

        // Create new task
        let task_id = self.task_registry.create_task_id();
        let task = AsyncTask {
            id: task_id,
            state: TaskState::Ready,
            priority: options.priority,
            function,
            created_at: Position::new(0, 0, 0), // Position tracked from function definition
            dependencies: Vec::new(),
            waker: None,
            frame,
            frame_hints: hints,
        };

        // Register task
        self.task_registry.register_task(task);
        self.scheduler.schedule_task(task_id, options.priority);

        TaskHandle::new(task_id)
    }

    /// Execute async function and return a Promise
    pub fn execute_async_function(&mut self, function: Box<dyn AsyncFunction>) -> Promise {
        let task_handle = self.spawn_task(function, TaskPriority::Normal);

        match task_handle.task_id() {
            Some(task_id) => {
                let promise = Promise::new(task_id);
                self.promise_resolver
                    .register_promise(task_id, promise.clone());
                promise
            }
            None => Promise::rejected("Failed to spawn async task".to_string()),
        }
    }

    /// Block the runtime until the specified task completes and return its result.
    pub fn wait_for_task(&mut self, task_id: TaskId) -> AsyncResult {
        loop {
            if let Some(result) = self.task_registry.take_result(task_id) {
                self.task_registry.finalize_task(task_id);
                return result;
            }

            if self.task_registry.get_task(task_id).is_none() && !self.has_pending_tasks() {
                return Err(AsyncError::TaskNotFound { task_id });
            }

            self.run_single_iteration()?;
        }
    }

    /// Attempt to cancel a task. Returns true if cancellation was scheduled, false if already completed.
    pub fn cancel_task(&mut self, task_id: TaskId) -> Result<bool, AsyncError> {
        if self.task_registry.get_result(task_id).is_some() {
            return Ok(false);
        }

        if self.task_registry.get_task(task_id).is_none() {
            return Err(AsyncError::TaskNotFound { task_id });
        }

        self.scheduler.cancel_task(task_id);
        if let Some(task) = self.task_registry.get_task_mut(task_id) {
            task.state = TaskState::Cancelled;
        }
        self.task_registry
            .store_result(task_id, Err(AsyncError::TaskCancelled { task_id }));
        Ok(true)
    }

    /// Run the event loop until completion
    pub fn run_until_complete(&mut self) -> AsyncResult {
        while self.has_pending_tasks() {
            self.run_single_iteration()?;
        }

        Ok(AsyncValue::Unit)
    }

    /// Run a single iteration of the event loop
    pub fn run_single_iteration(&mut self) -> AsyncResult {
        self.process_wake_queue();
        // Get next task to execute
        if let Some(task_id) = self.scheduler.get_next_task() {
            self.execute_task(task_id)?;
        }

        // Process completed tasks
        self.process_completed_tasks();

        // Clean up finished tasks if needed
        if self.task_registry.completed_task_count() > self.config.task_cleanup_threshold {
            self.cleanup_completed_tasks();
        }

        Ok(AsyncValue::Unit)
    }

    /// Execute a specific task
    fn execute_task(&mut self, task_id: TaskId) -> AsyncResult {
        let frame_parts = {
            let task = match self.task_registry.get_task_mut(task_id) {
                Some(task) => task,
                None => return Err(AsyncError::TaskNotFound { task_id }),
            };
            task.state = TaskState::Running;
            task.frame.checkout()
        };

        // Set up execution context seeded from the coroutine frame.
        let mut context = AsyncExecutionContext::from_parts(
            task_id,
            Arc::new(Mutex::new(AsyncRuntime::new())),
            frame_parts,
        );

        // Get function reference and execute
        let future = {
            let task = self
                .task_registry
                .get_task(task_id)
                .ok_or(AsyncError::TaskNotFound { task_id })?;
            task.function.execute(&mut context)
        };

        let poll_result = self.poll_future(future, task_id);

        // Return frame parts to the task for reuse (or recycling when ready).
        let frame_parts = context.into_parts();
        if let Some(task) = self.task_registry.get_task_mut(task_id) {
            task.frame.restore(frame_parts);
        }

        match poll_result {
            Poll::Ready(result) => {
                if let Some(task) = self.task_registry.get_task_mut(task_id) {
                    task.state = TaskState::Completed;
                    let recycled_frame = std::mem::replace(
                        &mut task.frame,
                        CoroutineFrameData::new_heap(task.frame_hints.clone()),
                    );
                    ScopeStack::recycle_frame(recycled_frame);
                }
                self.task_registry.store_result(task_id, result.clone());
                self.promise_resolver
                    .resolve_promise(task_id, result.clone());
                result
            }
            Poll::Pending => {
                if let Some(task) = self.task_registry.get_task_mut(task_id) {
                    task.state = TaskState::Waiting;
                }
                self.scheduler
                    .mark_task_waiting(task_id, TaskState::Waiting);
                Ok(AsyncValue::Pending)
            }
        }
    }

    /// Poll a future to completion or pending
    fn poll_future(
        &mut self,
        mut future: Pin<Box<dyn Future<Output = AsyncResult> + Send>>,
        task_id: TaskId,
    ) -> Poll<AsyncResult> {
        // Create waker for the task
        let waker = self.create_waker(task_id);
        let mut context = Context::from_waker(&waker);

        // Poll the future
        future.as_mut().poll(&mut context)
    }

    /// Create a waker for a task
    fn create_waker(&self, task_id: TaskId) -> Waker {
        use std::task::{RawWaker, RawWakerVTable};

        unsafe fn clone_fn(data: *const ()) -> RawWaker {
            let arc = Arc::<TaskWakeHandle>::from_raw(data as *const TaskWakeHandle);
            let cloned = Arc::clone(&arc);
            let _ = Arc::into_raw(arc);
            RawWaker::new(Arc::into_raw(cloned) as *const (), &VTABLE)
        }

        unsafe fn wake_fn(data: *const ()) {
            let arc = Arc::<TaskWakeHandle>::from_raw(data as *const TaskWakeHandle);
            arc.wake();
            // Ownership of the handle was transferred, so dropping `arc` here is correct.
        }

        unsafe fn wake_by_ref_fn(data: *const ()) {
            let arc = Arc::<TaskWakeHandle>::from_raw(data as *const TaskWakeHandle);
            arc.wake();
            let _ = Arc::into_raw(arc);
        }

        unsafe fn drop_fn(data: *const ()) {
            let _ = Arc::<TaskWakeHandle>::from_raw(data as *const TaskWakeHandle);
        }

        static VTABLE: RawWakerVTable =
            RawWakerVTable::new(clone_fn, wake_fn, wake_by_ref_fn, drop_fn);

        let handle = Arc::new(TaskWakeHandle::new(task_id, Arc::clone(&self.wake_queue)));
        let raw_waker = RawWaker::new(Arc::into_raw(handle) as *const (), &VTABLE);
        unsafe { Waker::from_raw(raw_waker) }
    }

    /// Promote tasks that were awoken by channel operations or futures.
    fn process_wake_queue(&mut self) {
        let pending = if let Ok(mut queue) = self.wake_queue.lock() {
            queue.drain(..).collect::<Vec<_>>()
        } else {
            Vec::new()
        };

        for task_id in pending {
            self.scheduler.wake_task(task_id);
        }
    }

    /// Process completed tasks and resolve promises
    fn process_completed_tasks(&mut self) {
        let completed_tasks: Vec<TaskId> = self.task_registry.get_completed_tasks();

        for task_id in completed_tasks {
            if let Some(result) = self.task_registry.get_result(task_id) {
                self.promise_resolver
                    .resolve_promise(task_id, result.clone());

                // Process promise chains
                if let Some(chain) = self.promise_resolver.get_promise_chain(task_id) {
                    for &chained_task_id in chain {
                        self.scheduler
                            .schedule_task(chained_task_id, TaskPriority::Normal);
                    }
                }
            }
        }
    }

    /// Clean up completed tasks to free memory
    fn cleanup_completed_tasks(&mut self) {
        self.task_registry.cleanup_completed_tasks();
        self.promise_resolver.cleanup_resolved_promises();
    }

    /// Check if there are pending tasks
    fn has_pending_tasks(&self) -> bool {
        self.scheduler.has_ready_tasks()
            || self.scheduler.has_waiting_tasks()
            || !self.promise_resolver.pending_promises.is_empty()
    }

    /// Get current configuration
    pub fn config(&self) -> &AsyncRuntimeConfig {
        &self.config
    }

    /// Update configuration
    pub fn set_config(&mut self, config: AsyncRuntimeConfig) {
        self.config = config;
    }

    /// Create a future that resolves when a send operation completes.
    pub fn channel_send_future(
        &self,
        channel: Channel,
        value: AsyncValue,
    ) -> Pin<Box<dyn Future<Output = AsyncResult> + Send>> {
        Box::pin(channel.send_future(value))
    }

    /// Create a future that resolves with the next value received from a channel.
    pub fn channel_receive_future(
        &self,
        channel: Channel,
    ) -> Pin<Box<dyn Future<Output = AsyncResult> + Send>> {
        Box::pin(channel.receive_future())
    }

    /// Create a future that resolves when a select expression completes.
    pub fn channel_select_future(
        &self,
        manager: &ChannelManager,
        operations: &[SelectCase],
        timeout: Option<Duration>,
    ) -> Result<ManagerSelectFuture, AsyncError> {
        manager.select_future(operations, timeout)
    }

    /// Snapshot runtime metrics for diagnostics/testing.
    pub fn metrics_snapshot(&self) -> RuntimeMetricsSnapshot {
        let frame = self.metrics.snapshot();
        let scheduler = self.scheduler.metrics_snapshot();
        RuntimeMetricsSnapshot {
            stack_frames_allocated: frame.stack_frames_allocated,
            stack_frames_reused: frame.stack_frames_reused,
            stack_frame_fallbacks: frame.stack_frame_fallbacks,
            heap_frames_allocated: frame.heap_frames_allocated,
            scheduler,
        }
    }

    /// Enter a scoped task arena for stack allocations.
    pub fn enter_task_scope(kind: TaskScopeKind, stack_budget_bytes: usize) -> TaskScopeGuard {
        TaskScopeGuard::new(kind, stack_budget_bytes)
    }

    fn allocate_frame(&self, hints: &CoroutineFrameHints) -> CoroutineFrameData {
        if matches!(hints.escape_class(), EscapeClass::ScopeBound) {
            if let Some((frame, reused)) =
                ScopeStack::with_current(|scope| scope.checkout_frame(hints))
            {
                self.metrics.record_stack_allocation(reused);
                return frame;
            }
            self.metrics.record_stack_fallback();
        }

        let (frame, reused) = checkout_heap_frame(hints);
        if !reused {
            self.metrics.record_heap_allocation();
        }
        frame
    }
}

impl Default for AsyncRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskScheduler {
    /// Create a new task scheduler
    pub fn new() -> Self {
        Self {
            ready_queue: VecDeque::new(),
            priority_queue: VecDeque::new(),
            waiting_tasks: HashMap::new(),
            current_task: None,
            metrics: SchedulerMetrics::default(),
            idle_polls_since_task: 0,
            queued_at: HashMap::new(),
        }
    }

    /// Schedule a task for execution
    pub fn schedule_task(&mut self, task_id: TaskId, priority: TaskPriority) {
        match priority {
            TaskPriority::High => {
                self.priority_queue.push_front((priority, task_id));
                self.metrics.scheduled_high += 1;
            }
            TaskPriority::Normal => {
                self.ready_queue.push_back((priority, task_id));
                self.metrics.scheduled_normal += 1;
            }
            TaskPriority::Low => {
                self.ready_queue.push_back((priority, task_id));
                self.metrics.scheduled_low += 1;
            }
        }
        self.queued_at.insert(task_id, Instant::now());
    }

    /// Get the next task to execute
    pub fn get_next_task(&mut self) -> Option<TaskId> {
        // Check priority queue first
        if let Some((_, task_id)) = self.priority_queue.pop_front() {
            self.current_task = Some(task_id);
            self.metrics.dispatched_high += 1;
            self.idle_polls_since_task = 0;
            self.record_latency_for(task_id);
            return Some(task_id);
        }

        // Then check ready queue
        if let Some((priority, task_id)) = self.ready_queue.pop_front() {
            self.current_task = Some(task_id);
            match priority {
                TaskPriority::High => {
                    self.metrics.dispatched_high += 1;
                }
                TaskPriority::Normal => {
                    self.metrics.dispatched_normal += 1;
                }
                TaskPriority::Low => {
                    self.metrics.dispatched_low += 1;
                }
            }
            self.idle_polls_since_task = 0;
            self.record_latency_for(task_id);
            return Some(task_id);
        }

        self.idle_polls_since_task = self.idle_polls_since_task.saturating_add(1);
        self.metrics.idle_polls += 1;
        if self.idle_polls_since_task >= SCHEDULER_BACKOFF_THRESHOLD {
            std::thread::yield_now();
            self.metrics.idle_backoffs += 1;
            self.idle_polls_since_task = 0;
        }
        None
    }

    /// Mark a task as waiting
    pub fn mark_task_waiting(&mut self, task_id: TaskId, state: TaskState) {
        self.waiting_tasks.insert(task_id, state);
        self.queued_at.remove(&task_id);
    }

    /// Wake up a waiting task
    pub fn wake_task(&mut self, task_id: TaskId) {
        if self.waiting_tasks.remove(&task_id).is_some() {
            self.ready_queue.push_back((TaskPriority::Normal, task_id));
            self.metrics.wake_promotions += 1;
            self.idle_polls_since_task = 0;
            self.queued_at.insert(task_id, Instant::now());
        }
    }

    /// Remove a task from scheduling queues
    pub fn cancel_task(&mut self, task_id: TaskId) {
        self.ready_queue.retain(|(_, id)| *id != task_id);
        self.priority_queue.retain(|(_, id)| *id != task_id);
        self.waiting_tasks.remove(&task_id);
        self.queued_at.remove(&task_id);
        if self.current_task == Some(task_id) {
            self.current_task = None;
        }
    }

    /// Check if there are ready tasks
    pub fn has_ready_tasks(&self) -> bool {
        !self.ready_queue.is_empty() || !self.priority_queue.is_empty()
    }

    /// Check if there are waiting tasks
    pub fn has_waiting_tasks(&self) -> bool {
        !self.waiting_tasks.is_empty()
    }

    pub fn metrics_snapshot(&self) -> SchedulerMetricsSnapshot {
        self.metrics.snapshot()
    }

    fn record_latency_for(&mut self, task_id: TaskId) {
        if let Some(enqueued) = self.queued_at.remove(&task_id) {
            let latency = enqueued.elapsed();
            self.metrics.wake_latency_samples += 1;
            self.metrics.wake_latency_ns_total = self
                .metrics
                .wake_latency_ns_total
                .saturating_add(latency.as_nanos());
            if latency.as_nanos() > self.metrics.wake_latency_max_ns {
                self.metrics.wake_latency_max_ns = latency.as_nanos();
            }
            if latency > STARVATION_THRESHOLD {
                self.metrics.starvation_events += 1;
            }
        }
    }
}

impl TaskRegistry {
    /// Create a new task registry
    pub fn new() -> Self {
        Self {
            slots: Vec::new(),
            free_list: Vec::new(),
        }
    }

    /// Allocate a new task handle (slot + generation)
    pub fn create_task_id(&mut self) -> TaskId {
        let slot = if let Some(slot) = self.free_list.pop() {
            slot
        } else {
            let slot = self.slots.len() as u32;
            self.slots.push(TaskSlot::new());
            slot
        };

        let generation = self.slots[slot as usize].generation;
        TaskId::new(slot, generation)
    }

    /// Register a task in the arena
    pub fn register_task(&mut self, task: AsyncTask) {
        let slot_idx = task.id.slot() as usize;
        let generation = task.id.generation();

        let slot = self
            .slots
            .get_mut(slot_idx)
            .expect("task slot missing during registration");

        if slot.generation != generation {
            panic!("attempted to register task with stale handle");
        }

        if slot.task.is_some() {
            panic!("task slot already occupied");
        }

        slot.task = Some(task);
    }

    /// Get an immutable reference to a task if the handle is current
    pub fn get_task(&self, task_id: TaskId) -> Option<&AsyncTask> {
        self.slot(task_id).and_then(|slot| slot.task.as_ref())
    }

    /// Get a mutable reference to a task if the handle is current
    pub fn get_task_mut(&mut self, task_id: TaskId) -> Option<&mut AsyncTask> {
        self.slot_mut(task_id).and_then(|slot| slot.task.as_mut())
    }

    /// Store the result for a completed task
    pub fn store_result(&mut self, task_id: TaskId, result: AsyncResult) {
        if let Some(slot) = self.slot_mut(task_id) {
            slot.result = Some(result);
        }
    }

    /// Fetch the result for a task handle
    pub fn get_result(&self, task_id: TaskId) -> Option<&AsyncResult> {
        self.slot(task_id).and_then(|slot| slot.result.as_ref())
    }

    /// Take (and remove) the result for a task handle
    pub fn take_result(&mut self, task_id: TaskId) -> Option<AsyncResult> {
        self.slot_mut(task_id).and_then(|slot| slot.result.take())
    }

    /// Finalize a task slot after its result has been consumed
    pub fn finalize_task(&mut self, task_id: TaskId) {
        if let Some(slot) = self.slot_mut(task_id) {
            slot.task = None;
            slot.result = None;
            slot.generation = slot.generation.wrapping_add(1);
            self.free_list.push(task_id.slot());
        }
    }

    /// Return all task IDs that have completed and still have a stored result
    pub fn get_completed_tasks(&self) -> Vec<TaskId> {
        self.slots
            .iter()
            .enumerate()
            .filter_map(|(idx, slot)| {
                if slot
                    .task
                    .as_ref()
                    .map_or(false, |task| matches!(task.state, TaskState::Completed))
                    && slot.result.is_some()
                {
                    Some(TaskId::new(idx as u32, slot.generation))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Count active tasks
    pub fn active_task_count(&self) -> usize {
        self.slots
            .iter()
            .filter(|slot| {
                slot.task
                    .as_ref()
                    .map_or(false, |task| !matches!(task.state, TaskState::Completed))
            })
            .count()
    }

    /// Count completed tasks that still retain results
    pub fn completed_task_count(&self) -> usize {
        self.slots
            .iter()
            .filter(|slot| slot.result.is_some())
            .count()
    }

    /// Remove completed tasks, bumping generation to invalidate stale handles
    pub fn cleanup_completed_tasks(&mut self) {
        for (idx, slot) in self.slots.iter_mut().enumerate() {
            let should_cleanup = slot
                .task
                .as_ref()
                .map_or(false, |task| matches!(task.state, TaskState::Completed))
                && slot.result.is_some();

            if should_cleanup {
                slot.task = None;
                slot.result = None;
                slot.generation = slot.generation.wrapping_add(1);
                self.free_list.push(idx as u32);
            }
        }
    }

    fn slot(&self, task_id: TaskId) -> Option<&TaskSlot> {
        self.slots.get(task_id.slot() as usize).and_then(|slot| {
            if slot.generation == task_id.generation() {
                Some(slot)
            } else {
                None
            }
        })
    }

    fn slot_mut(&mut self, task_id: TaskId) -> Option<&mut TaskSlot> {
        self.slots
            .get_mut(task_id.slot() as usize)
            .and_then(|slot| {
                if slot.generation == task_id.generation() {
                    Some(slot)
                } else {
                    None
                }
            })
    }
}

impl PromiseResolver {
    /// Create a new promise resolver
    pub fn new() -> Self {
        Self {
            pending_promises: HashMap::new(),
            promise_chains: HashMap::new(),
            resolved_values: HashMap::new(),
        }
    }

    /// Register a promise for resolution
    pub fn register_promise(&mut self, task_id: TaskId, promise: Promise) {
        self.pending_promises.insert(task_id, promise);
    }

    /// Resolve a promise with a value
    pub fn resolve_promise(&mut self, task_id: TaskId, result: AsyncResult) {
        if let Some(mut promise) = self.pending_promises.remove(&task_id) {
            match &result {
                Ok(value) => promise.resolve(value.clone()),
                Err(error) => promise.reject(format!("{:?}", error)),
            }

            // Store resolved value
            match result {
                Ok(value) => {
                    self.resolved_values.insert(task_id, value);
                }
                Err(_) => {
                    // Store error as a special async value
                    self.resolved_values.insert(task_id, AsyncValue::Error);
                }
            }
        }
    }

    /// Add a promise chain (for .then() operations)
    pub fn add_promise_chain(&mut self, parent_task: TaskId, child_task: TaskId) {
        self.promise_chains
            .entry(parent_task)
            .or_insert_with(Vec::new)
            .push(child_task);
    }

    /// Get promise chain for a task
    pub fn get_promise_chain(&self, task_id: TaskId) -> Option<&Vec<TaskId>> {
        self.promise_chains.get(&task_id)
    }

    /// Clean up resolved promises
    pub fn cleanup_resolved_promises(&mut self) {
        // Keep only unresolved promises
        let resolved_tasks: Vec<TaskId> = self.resolved_values.keys().cloned().collect();
        for task_id in resolved_tasks {
            self.promise_chains.remove(&task_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct TestAsyncFunction {
        name: String,
        result: AsyncValue,
    }

    impl AsyncFunction for TestAsyncFunction {
        fn execute(
            &self,
            _context: &mut AsyncExecutionContext,
        ) -> Pin<Box<dyn Future<Output = AsyncResult> + Send>> {
            let result = Ok(self.result.clone());
            Box::pin(async move { result })
        }

        fn name(&self) -> &str {
            &self.name
        }

        fn is_pure(&self) -> bool {
            true
        }
    }

    #[test]
    fn test_async_runtime_creation() {
        let runtime = AsyncRuntime::new();
        assert_eq!(runtime.config.max_concurrent_tasks, 1000);
        assert!(runtime.config.enable_priority_scheduling);
    }

    #[test]
    fn test_task_spawning() {
        let mut runtime = AsyncRuntime::new();

        let function = Box::new(TestAsyncFunction {
            name: "test_function".to_string(),
            result: AsyncValue::Integer(42),
        });

        let handle = runtime.spawn_task(function, TaskPriority::Normal);
        assert!(handle.task_id().is_some());
    }

    #[test]
    fn test_task_scheduler() {
        let mut scheduler = TaskScheduler::new();

        let task_id = TaskId::new(1, 0);
        scheduler.schedule_task(task_id, TaskPriority::High);

        assert!(scheduler.has_ready_tasks());
        assert_eq!(scheduler.get_next_task(), Some(task_id));
    }

    #[test]
    fn test_task_registry() {
        let mut registry = TaskRegistry::new();

        let task_id = registry.create_task_id();
        assert_eq!(task_id.slot(), 0);
        assert_eq!(task_id.generation(), 0);

        let second_id = registry.create_task_id();
        assert_eq!(second_id.slot(), 1);
        assert_eq!(second_id.generation(), 0);
    }

    #[test]
    fn task_registry_invalidates_stale_handles() {
        let mut registry = TaskRegistry::new();
        let task_id = registry.create_task_id();

        let task = AsyncTask {
            id: task_id,
            state: TaskState::Completed,
            priority: TaskPriority::Normal,
            function: Box::new(TestAsyncFunction {
                name: "noop".to_string(),
                result: AsyncValue::Unit,
            }),
            created_at: Position::new(0, 0, 0),
            dependencies: Vec::new(),
            waker: None,
            frame: CoroutineFrameData::new_heap(CoroutineFrameHints::default()),
            frame_hints: CoroutineFrameHints::default(),
        };

        registry.register_task(task);
        registry.store_result(task_id, Ok(AsyncValue::Unit));
        assert!(registry.get_task(task_id).is_some());

        registry.cleanup_completed_tasks();

        assert!(registry.get_task(task_id).is_none());

        let recycled_id = registry.create_task_id();
        assert_eq!(recycled_id.slot(), task_id.slot());
        assert_ne!(recycled_id.generation(), task_id.generation());
    }

    #[test]
    fn test_promise_resolver() {
        let mut resolver = PromiseResolver::new();

        let task_id = TaskId::new(1, 0);
        let promise = Promise::new(task_id);

        resolver.register_promise(task_id, promise);
        assert!(resolver.pending_promises.contains_key(&task_id));

        resolver.resolve_promise(task_id, Ok(AsyncValue::String("test".to_string())));
        assert!(!resolver.pending_promises.contains_key(&task_id));
        assert!(resolver.resolved_values.contains_key(&task_id));
    }

    #[test]
    fn scope_bound_tasks_use_stack_frames() {
        let mut runtime = AsyncRuntime::new();
        let _scope = AsyncRuntime::enter_task_scope(TaskScopeKind::Async, 512);

        for i in 0..4 {
            let function = Box::new(TestAsyncFunction {
                name: format!("stack_task_{i}"),
                result: AsyncValue::Integer(i),
            });
            let handle = runtime.spawn_task_with_options(
                function,
                TaskSpawnOptions::scope_bound(TaskPriority::Normal, 512),
            );
            let task_id = handle.task_id().expect("task id");
            runtime
                .wait_for_task(task_id)
                .expect("stack task completes");
        }

        let snapshot = runtime.metrics_snapshot();
        assert!(
            snapshot.stack_frames_allocated >= 1,
            "expected stack frames to be allocated, got {:?}",
            snapshot
        );
        assert_eq!(
            snapshot.stack_frame_fallbacks, 0,
            "scope-bound tasks should not fall back to heap frames"
        );
    }

    #[test]
    fn stack_frames_reuse_within_scope() {
        let mut runtime = AsyncRuntime::new();
        let _scope = AsyncRuntime::enter_task_scope(TaskScopeKind::Async, 256);

        for i in 0..6 {
            let function = Box::new(TestAsyncFunction {
                name: format!("reuse_task_{i}"),
                result: AsyncValue::Integer(i * 2),
            });
            let handle = runtime.spawn_task_with_options(
                function,
                TaskSpawnOptions::scope_bound(TaskPriority::Normal, 256),
            );
            runtime
                .wait_for_task(handle.task_id().unwrap())
                .expect("reuse task completes");
        }

        let snapshot = runtime.metrics_snapshot();
        assert!(
            snapshot.stack_frames_reused >= 1,
            "expected reused frames, got {:?}",
            snapshot
        );
    }

    #[test]
    fn heap_fallback_occurs_without_scope() {
        let mut runtime = AsyncRuntime::new();
        let function = Box::new(TestAsyncFunction {
            name: "heap_only".to_string(),
            result: AsyncValue::Unit,
        });
        let handle = runtime.spawn_task_with_options(
            function,
            TaskSpawnOptions::scope_bound(TaskPriority::Normal, 128),
        );
        runtime
            .wait_for_task(handle.task_id().unwrap())
            .expect("heap task completes");

        let snapshot = runtime.metrics_snapshot();
        assert_eq!(snapshot.stack_frames_allocated, 0);
        assert_eq!(snapshot.stack_frames_reused, 0);
        assert!(
            snapshot.stack_frame_fallbacks >= 1,
            "expected fallback increment, got {:?}",
            snapshot
        );
        assert!(
            snapshot.heap_frames_allocated >= 1,
            "expected heap frame allocation, got {:?}",
            snapshot
        );
    }

    #[test]
    fn scheduler_backoff_counts_idle_polls() {
        let mut runtime = AsyncRuntime::new();
        for _ in 0..(SCHEDULER_BACKOFF_THRESHOLD + 2) {
            runtime
                .run_single_iteration()
                .expect("idle iteration executes");
        }
        let snapshot = runtime.metrics_snapshot();
        assert!(
            snapshot.scheduler.idle_backoffs >= 1,
            "expected at least one backoff, got {:?}",
            snapshot.scheduler
        );
        assert!(
            snapshot.scheduler.idle_polls >= SCHEDULER_BACKOFF_THRESHOLD as u64,
            "expected idle polls to be recorded, got {:?}",
            snapshot.scheduler
        );
    }

    #[test]
    fn scheduler_metrics_track_priority_dispatches() {
        let mut runtime = AsyncRuntime::new();

        let priorities = [TaskPriority::High, TaskPriority::Normal, TaskPriority::Low];

        for (idx, priority) in priorities.iter().enumerate() {
            let function = Box::new(TestAsyncFunction {
                name: format!("priority_task_{idx}"),
                result: AsyncValue::Unit,
            });
            let handle = runtime.spawn_task(function, *priority);
            runtime
                .wait_for_task(handle.task_id().unwrap())
                .expect("priority task completes");
        }

        let snapshot = runtime.metrics_snapshot();
        assert!(
            snapshot.scheduler.dispatched_high >= 1,
            "high priority dispatch missing: {:?}",
            snapshot.scheduler
        );
        assert!(
            snapshot.scheduler.dispatched_normal >= 1,
            "normal priority dispatch missing: {:?}",
            snapshot.scheduler
        );
        assert!(
            snapshot.scheduler.dispatched_low >= 1,
            "low priority dispatch missing: {:?}",
            snapshot.scheduler
        );
    }

    #[test]
    fn wake_latency_metrics_record_starvation() {
        let mut runtime = AsyncRuntime::new();
        let function = Box::new(TestAsyncFunction {
            name: "latency_task".to_string(),
            result: AsyncValue::Unit,
        });
        let handle = runtime.spawn_task(function, TaskPriority::Normal);

        // Simulate a delayed wake by sleeping before running the scheduler.
        std::thread::sleep(Duration::from_millis(10));
        runtime
            .run_single_iteration()
            .expect("latency iteration runs");
        runtime
            .wait_for_task(handle.task_id().unwrap())
            .expect("latency task completes");

        let snapshot = runtime.metrics_snapshot();
        assert!(
            snapshot.scheduler.wake_latency_ns_total > 0,
            "expected wake latency to accumulate, got {:?}",
            snapshot.scheduler
        );
        assert!(
            snapshot.scheduler.starvation_events >= 1,
            "expected starvation detection, got {:?}",
            snapshot.scheduler
        );
    }
}
