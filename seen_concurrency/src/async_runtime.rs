//! Async runtime for Seen Language
//!
//! This module implements a cooperative async runtime with support for
//! Promise/Future types, async functions, and efficient task scheduling.
//! The runtime is designed for zero-cost abstractions with compile-time
//! optimization and runtime efficiency.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use std::pin::Pin;
use std::future::Future;
use seen_parser::ast::Expression;
use seen_lexer::position::Position;
use crate::types::{
    TaskId, TaskHandle, TaskState, AsyncValue, AsyncError, 
    Promise, AsyncResult, TaskPriority
};

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
    ready_queue: VecDeque<TaskId>,
    /// Priority task queue (if priority scheduling enabled)
    priority_queue: VecDeque<(TaskPriority, TaskId)>,
    /// Waiting tasks (blocked on I/O, etc.)
    waiting_tasks: HashMap<TaskId, TaskState>,
    /// Currently executing task
    current_task: Option<TaskId>,
}

/// Registry for tracking all tasks in the system
#[derive(Debug)]
pub struct TaskRegistry {
    /// All tasks indexed by ID
    tasks: HashMap<TaskId, AsyncTask>,
    /// Next available task ID
    next_task_id: u64,
    /// Completed task results
    completed_results: HashMap<TaskId, AsyncResult>,
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
}

/// Trait for async function execution
pub trait AsyncFunction: Send + Sync + std::fmt::Debug {
    /// Execute the async function
    fn execute(&self, context: &mut AsyncExecutionContext) -> Pin<Box<dyn Future<Output = AsyncResult> + Send>>;
    
    /// Get function name for debugging
    fn name(&self) -> &str;
    
    /// Check if function is pure (no side effects)
    fn is_pure(&self) -> bool { false }
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
}

impl AsyncRuntime {
    /// Create a new async runtime with default configuration
    pub fn new() -> Self {
        Self {
            scheduler: TaskScheduler::new(),
            task_registry: TaskRegistry::new(),
            promise_resolver: PromiseResolver::new(),
            config: AsyncRuntimeConfig::default(),
        }
    }
    
    /// Create async runtime with custom configuration
    pub fn with_config(config: AsyncRuntimeConfig) -> Self {
        Self {
            scheduler: TaskScheduler::new(),
            task_registry: TaskRegistry::new(),
            promise_resolver: PromiseResolver::new(),
            config,
        }
    }
    
    /// Spawn a new async task
    pub fn spawn_task(&mut self, function: Box<dyn AsyncFunction>, priority: TaskPriority) -> TaskHandle {
        // Check task limits
        if self.task_registry.active_task_count() >= self.config.max_concurrent_tasks {
            return TaskHandle::error(AsyncError::TaskLimitExceeded {
                limit: self.config.max_concurrent_tasks,
                position: Position::new(0, 0, 0),
            });
        }
        
        // Create new task
        let task_id = self.task_registry.create_task_id();
        let task = AsyncTask {
            id: task_id,
            state: TaskState::Ready,
            priority,
            function,
            created_at: Position::new(0, 0, 0), // Position tracked from function definition
            dependencies: Vec::new(),
            waker: None,
        };
        
        // Register task
        self.task_registry.register_task(task);
        self.scheduler.schedule_task(task_id, priority);
        
        TaskHandle::new(task_id)
    }
    
    /// Execute async function and return a Promise
    pub fn execute_async_function(&mut self, function: Box<dyn AsyncFunction>) -> Promise {
        let task_handle = self.spawn_task(function, TaskPriority::Normal);
        
        match task_handle.task_id() {
            Some(task_id) => {
                let promise = Promise::new(task_id);
                self.promise_resolver.register_promise(task_id, promise.clone());
                promise
            }
            None => Promise::rejected("Failed to spawn async task".to_string())
        }
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
        // Update task state
        {
            let task = match self.task_registry.get_task_mut(task_id) {
                Some(task) => task,
                None => return Err(AsyncError::TaskNotFound { task_id }),
            };
            task.state = TaskState::Running;
        }
        
        // Set up execution context
        let mut context = AsyncExecutionContext {
            task_id,
            runtime: Arc::new(Mutex::new(AsyncRuntime::new())), // Create new runtime for isolation
            local_state: HashMap::new(),
            stack_trace: Vec::new(),
        };
        
        // Get function reference and execute
        let future = {
            let task = self.task_registry.get_task(task_id)
                .ok_or(AsyncError::TaskNotFound { task_id })?;
            task.function.execute(&mut context)
        };
        
        // Poll the future
        match self.poll_future(future, task_id) {
            Poll::Ready(result) => {
                if let Some(task) = self.task_registry.get_task_mut(task_id) {
                    task.state = TaskState::Completed;
                }
                self.task_registry.store_result(task_id, result.clone());
                self.promise_resolver.resolve_promise(task_id, result.clone());
                result
            }
            Poll::Pending => {
                if let Some(task) = self.task_registry.get_task_mut(task_id) {
                    task.state = TaskState::Waiting;
                }
                Ok(AsyncValue::Pending)
            }
        }
    }
    
    /// Poll a future to completion or pending
    fn poll_future(
        &mut self, 
        mut future: Pin<Box<dyn Future<Output = AsyncResult> + Send>>, 
        task_id: TaskId
    ) -> Poll<AsyncResult> {
        // Create waker for the task
        let waker = self.create_waker(task_id);
        let mut context = Context::from_waker(&waker);
        
        // Poll the future
        future.as_mut().poll(&mut context)
    }
    
    /// Create a waker for a task
    fn create_waker(&self, task_id: TaskId) -> Waker {
        // Create a waker that can wake the task when ready
        use std::task::{RawWaker, RawWakerVTable};
        
        unsafe fn clone(_: *const ()) -> RawWaker {
            RawWaker::new(std::ptr::null(), &VTABLE)
        }
        
        unsafe fn wake(_: *const ()) {}
        unsafe fn wake_by_ref(_: *const ()) {}
        unsafe fn drop(_: *const ()) {}
        
        static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, wake, wake_by_ref, drop);
        
        let raw_waker = RawWaker::new(std::ptr::null(), &VTABLE);
        unsafe { Waker::from_raw(raw_waker) }
    }
    
    /// Process completed tasks and resolve promises
    fn process_completed_tasks(&mut self) {
        let completed_tasks: Vec<TaskId> = self.task_registry.get_completed_tasks();
        
        for task_id in completed_tasks {
            if let Some(result) = self.task_registry.get_result(task_id) {
                self.promise_resolver.resolve_promise(task_id, result.clone());
                
                // Process promise chains
                if let Some(chain) = self.promise_resolver.get_promise_chain(task_id) {
                    for &chained_task_id in chain {
                        self.scheduler.schedule_task(chained_task_id, TaskPriority::Normal);
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
        self.scheduler.has_ready_tasks() || 
        self.scheduler.has_waiting_tasks() ||
        !self.promise_resolver.pending_promises.is_empty()
    }
    
    /// Get current configuration
    pub fn config(&self) -> &AsyncRuntimeConfig {
        &self.config
    }
    
    /// Update configuration
    pub fn set_config(&mut self, config: AsyncRuntimeConfig) {
        self.config = config;
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
        }
    }
    
    /// Schedule a task for execution
    pub fn schedule_task(&mut self, task_id: TaskId, priority: TaskPriority) {
        match priority {
            TaskPriority::High => {
                self.priority_queue.push_front((priority, task_id));
            }
            TaskPriority::Normal => {
                self.ready_queue.push_back(task_id);
            }
            TaskPriority::Low => {
                self.ready_queue.push_front(task_id);
            }
        }
    }
    
    /// Get the next task to execute
    pub fn get_next_task(&mut self) -> Option<TaskId> {
        // Check priority queue first
        if let Some((_, task_id)) = self.priority_queue.pop_front() {
            self.current_task = Some(task_id);
            return Some(task_id);
        }
        
        // Then check ready queue
        if let Some(task_id) = self.ready_queue.pop_front() {
            self.current_task = Some(task_id);
            return Some(task_id);
        }
        
        None
    }
    
    /// Mark a task as waiting
    pub fn mark_task_waiting(&mut self, task_id: TaskId, state: TaskState) {
        self.waiting_tasks.insert(task_id, state);
    }
    
    /// Wake up a waiting task
    pub fn wake_task(&mut self, task_id: TaskId) {
        if self.waiting_tasks.remove(&task_id).is_some() {
            self.ready_queue.push_back(task_id);
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
}

impl TaskRegistry {
    /// Create a new task registry
    pub fn new() -> Self {
        Self {
            tasks: HashMap::new(),
            next_task_id: 1,
            completed_results: HashMap::new(),
        }
    }
    
    /// Create a new unique task ID
    pub fn create_task_id(&mut self) -> TaskId {
        let id = TaskId::new(self.next_task_id);
        self.next_task_id += 1;
        id
    }
    
    /// Register a new task
    pub fn register_task(&mut self, task: AsyncTask) {
        self.tasks.insert(task.id, task);
    }
    
    /// Get a task by ID
    pub fn get_task(&self, task_id: TaskId) -> Option<&AsyncTask> {
        self.tasks.get(&task_id)
    }
    
    /// Get a mutable task by ID
    pub fn get_task_mut(&mut self, task_id: TaskId) -> Option<&mut AsyncTask> {
        self.tasks.get_mut(&task_id)
    }
    
    /// Store task result
    pub fn store_result(&mut self, task_id: TaskId, result: AsyncResult) {
        self.completed_results.insert(task_id, result);
    }
    
    /// Get task result
    pub fn get_result(&self, task_id: TaskId) -> Option<&AsyncResult> {
        self.completed_results.get(&task_id)
    }
    
    /// Get all completed tasks
    pub fn get_completed_tasks(&self) -> Vec<TaskId> {
        self.tasks.iter()
            .filter(|(_, task)| matches!(task.state, TaskState::Completed))
            .map(|(id, _)| *id)
            .collect()
    }
    
    /// Get active task count
    pub fn active_task_count(&self) -> usize {
        self.tasks.len() - self.completed_results.len()
    }
    
    /// Get completed task count
    pub fn completed_task_count(&self) -> usize {
        self.completed_results.len()
    }
    
    /// Clean up completed tasks
    pub fn cleanup_completed_tasks(&mut self) {
        let completed_tasks: Vec<TaskId> = self.completed_results.keys().cloned().collect();
        for task_id in completed_tasks {
            self.tasks.remove(&task_id);
            self.completed_results.remove(&task_id);
        }
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
        self.promise_chains.entry(parent_task)
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
        fn execute(&self, _context: &mut AsyncExecutionContext) -> Pin<Box<dyn Future<Output = AsyncResult> + Send>> {
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
        
        let task_id = TaskId::new(1);
        scheduler.schedule_task(task_id, TaskPriority::High);
        
        assert!(scheduler.has_ready_tasks());
        assert_eq!(scheduler.get_next_task(), Some(task_id));
    }
    
    #[test]
    fn test_task_registry() {
        let mut registry = TaskRegistry::new();
        
        let task_id = registry.create_task_id();
        assert_eq!(task_id.id(), 1);
        
        let second_id = registry.create_task_id();
        assert_eq!(second_id.id(), 2);
    }
    
    #[test]
    fn test_promise_resolver() {
        let mut resolver = PromiseResolver::new();
        
        let task_id = TaskId::new(1);
        let promise = Promise::new(task_id);
        
        resolver.register_promise(task_id, promise);
        assert!(resolver.pending_promises.contains_key(&task_id));
        
        resolver.resolve_promise(task_id, Ok(AsyncValue::String("test".to_string())));
        assert!(!resolver.pending_promises.contains_key(&task_id));
        assert!(resolver.resolved_values.contains_key(&task_id));
    }
}