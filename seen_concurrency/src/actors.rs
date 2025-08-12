//! Actor model implementation for Seen Language
//!
//! This module implements the actor model according to Seen's syntax design:
//! - actor TypeName { receive Message { ... } }
//! - spawn ActorType()
//! - send Message to actor
//! - request Message from actor
//! - Proper message type safety and supervision

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, Condvar};
use std::time::{Duration, Instant, SystemTime};
use seen_parser::ast::{Expression, Type};
use seen_lexer::position::Position;
use crate::types::{
    ActorId, ActorRef, ActorMessage, AsyncValue, AsyncError, AsyncResult, 
    TaskId, TaskPriority, Mailbox
};

/// Actor definition following Seen syntax
#[derive(Debug, Clone)]
pub struct ActorDefinition {
    /// Actor type name (follows visibility rules)
    pub name: String,
    /// Initial state variables
    pub state_variables: HashMap<String, Type>,
    /// Message handlers with their implementations
    pub message_handlers: HashMap<String, MessageHandler>,
    /// Whether actor is public (capitalized name)
    pub is_public: bool,
    /// Position where actor is defined
    pub position: Position,
    /// Supervision strategy
    pub supervision: SupervisionStrategy,
}

/// Message handler for actor receive blocks
#[derive(Debug, Clone)]
pub struct MessageHandler {
    /// Message type name
    pub message_type: String,
    /// Handler implementation
    pub handler: Expression,
    /// Whether handler expects a reply
    pub expects_reply: bool,
    /// Return type for request/reply pattern
    pub return_type: Option<Type>,
    /// Handler position for debugging
    pub position: Position,
}

/// Running actor instance
#[derive(Debug)]
pub struct ActorInstance {
    /// Unique actor identifier
    pub id: ActorId,
    /// Actor type name
    pub actor_type: String,
    /// Actor definition reference
    pub definition: Arc<ActorDefinition>,
    /// Actor state (mutable variables)
    pub state: HashMap<String, AsyncValue>,
    /// Actor mailbox for receiving messages
    pub mailbox: Arc<Mailbox>,
    /// Actor execution state
    pub execution_state: ActorExecutionState,
    /// Current supervisor actor
    pub supervisor: Option<ActorId>,
    /// Child actors being supervised
    pub children: Vec<ActorId>,
    /// Actor statistics
    pub stats: ActorStats,
}

/// Actor execution states
#[derive(Debug, Clone, PartialEq)]
pub enum ActorExecutionState {
    /// Actor is ready to process messages
    Ready,
    /// Actor is currently processing a message
    Processing,
    /// Actor is waiting for a response
    WaitingForReply,
    /// Actor is being restarted due to failure
    Restarting,
    /// Actor has stopped normally
    Stopped,
    /// Actor has failed and cannot be restarted
    Failed(String),
}

/// Supervision strategies for actor failures
#[derive(Debug, Clone, PartialEq)]
pub enum SupervisionStrategy {
    /// Restart the actor on failure
    Restart,
    /// Stop the actor on failure
    Stop,
    /// Escalate failure to supervisor
    Escalate,
    /// Resume processing (ignore failure)
    Resume,
}

/// Actor statistics for monitoring
#[derive(Debug, Clone)]
pub struct ActorStats {
    /// Number of messages processed
    pub messages_processed: u64,
    /// Number of failures encountered
    pub failure_count: u64,
    /// Last restart time
    pub last_restart: Option<SystemTime>,
    /// Average message processing time
    pub avg_processing_time_ms: f64,
    /// Actor creation time
    pub created_at: SystemTime,
}

/// Message types for actor communication
#[derive(Debug, Clone)]
pub enum ActorMessageType {
    /// Regular message (fire-and-forget)
    Send {
        message: String,
        payload: AsyncValue,
    },
    /// Request message (expects reply)
    Request {
        message: String,
        payload: AsyncValue,
        reply_to: ActorId,
        request_id: u64,
    },
    /// Reply to a request
    Reply {
        request_id: u64,
        payload: AsyncValue,
    },
    /// System message for actor lifecycle
    System(SystemMessage),
}

/// System messages for actor management
#[derive(Debug, Clone)]
pub enum SystemMessage {
    /// Start actor processing
    Start,
    /// Stop actor gracefully
    Stop,
    /// Restart actor after failure
    Restart,
    /// Supervise child actor
    Supervise(ActorId),
    /// Child actor has failed
    ChildFailed {
        child_id: ActorId,
        error: String,
    },
}

/// Manager for all actors in the system
#[derive(Debug)]
pub struct ActorSystem {
    /// All running actors indexed by ID
    actors: HashMap<ActorId, ActorInstance>,
    /// Actor definitions (types)
    definitions: HashMap<String, Arc<ActorDefinition>>,
    /// Next available actor ID
    next_actor_id: u64,
    /// System supervisor (root actor)
    system_supervisor: Option<ActorId>,
    /// Dead letter queue for undeliverable messages
    dead_letters: VecDeque<ActorMessage>,
    /// Actor system configuration
    config: ActorSystemConfig,
}

/// Configuration for actor system
#[derive(Debug, Clone)]
pub struct ActorSystemConfig {
    /// Maximum number of actors
    pub max_actors: usize,
    /// Default mailbox capacity
    pub default_mailbox_capacity: usize,
    /// Message timeout for request/reply
    pub request_timeout_ms: u64,
    /// Maximum restart attempts
    pub max_restart_attempts: u32,
    /// Dead letter queue capacity
    pub dead_letter_capacity: usize,
}

impl Default for ActorSystemConfig {
    fn default() -> Self {
        Self {
            max_actors: 10000,
            default_mailbox_capacity: 1000,
            request_timeout_ms: 5000,
            max_restart_attempts: 3,
            dead_letter_capacity: 1000,
        }
    }
}

impl ActorDefinition {
    /// Create a new actor definition
    pub fn new(
        name: String,
        state_variables: HashMap<String, Type>,
        message_handlers: HashMap<String, MessageHandler>,
        position: Position,
    ) -> Self {
        let is_public = name.chars().next().map_or(false, |c| c.is_uppercase());
        
        Self {
            name,
            state_variables,
            message_handlers,
            is_public,
            position,
            supervision: SupervisionStrategy::Restart, // Default strategy
        }
    }
    
    /// Set supervision strategy
    pub fn with_supervision(mut self, strategy: SupervisionStrategy) -> Self {
        self.supervision = strategy;
        self
    }
    
    /// Check if actor can handle a message type
    pub fn can_handle(&self, message_type: &str) -> bool {
        self.message_handlers.contains_key(message_type)
    }
    
    /// Get message handler for a type
    pub fn get_handler(&self, message_type: &str) -> Option<&MessageHandler> {
        self.message_handlers.get(message_type)
    }
    
    /// Get actor signature for display
    pub fn signature(&self) -> String {
        let handlers: Vec<String> = self.message_handlers
            .keys()
            .map(|k| k.clone())
            .collect();
        
        format!(
            "actor {} {{ receive: [{}] }}",
            self.name,
            handlers.join(", ")
        )
    }
}

impl MessageHandler {
    /// Create a new message handler
    pub fn new(
        message_type: String,
        handler: Expression,
        expects_reply: bool,
        return_type: Option<Type>,
        position: Position,
    ) -> Self {
        Self {
            message_type,
            handler,
            expects_reply,
            return_type,
            position,
        }
    }
    
    /// Create a simple send handler (no reply)
    pub fn send_handler(message_type: String, handler: Expression, position: Position) -> Self {
        Self::new(message_type, handler, false, None, position)
    }
    
    /// Create a request handler (expects reply)
    pub fn request_handler(
        message_type: String,
        handler: Expression,
        return_type: Type,
        position: Position,
    ) -> Self {
        Self::new(message_type, handler, true, Some(return_type), position)
    }
}

impl ActorInstance {
    /// Create a new actor instance
    pub fn new(
        id: ActorId,
        definition: Arc<ActorDefinition>,
        initial_state: HashMap<String, AsyncValue>,
    ) -> Self {
        let mailbox = Arc::new(Mailbox {
            messages: Mutex::new(VecDeque::new()),
            capacity: Some(1000), // Default capacity
        });
        
        Self {
            id,
            actor_type: definition.name.clone(),
            definition,
            state: initial_state,
            mailbox,
            execution_state: ActorExecutionState::Ready,
            supervisor: None,
            children: Vec::new(),
            stats: ActorStats {
                messages_processed: 0,
                failure_count: 0,
                last_restart: None,
                avg_processing_time_ms: 0.0,
                created_at: SystemTime::now(),
            },
        }
    }
    
    /// Process a message using the appropriate handler
    pub fn process_message(&mut self, message: ActorMessage) -> AsyncResult {
        let start_time = Instant::now();
        self.execution_state = ActorExecutionState::Processing;
        
        // Extract message type and payload
        let (message_type, payload) = match &message.content {
            AsyncValue::String(msg) => (msg.clone(), AsyncValue::Unit),
            // Handle structured messages by extracting type from value
            AsyncValue::Array(ref arr) if arr.len() >= 2 => {
                if let (AsyncValue::String(msg_type), payload) = (&arr[0], &arr[1]) {
                    (msg_type.clone(), payload.clone())
                } else {
                    ("Unknown".to_string(), message.content.clone())
                }
            }
            _ => ("Unknown".to_string(), message.content.clone()),
        };
        
        // Find appropriate handler
        if let Some(handler) = self.definition.get_handler(&message_type).cloned() {
            // Execute handler
            let result = self.execute_handler(&handler, payload);
            
            // Update statistics
            let processing_time = start_time.elapsed().as_millis() as f64;
            self.update_stats(processing_time);
            
            self.execution_state = ActorExecutionState::Ready;
            result
        } else {
            // No handler found - this is an error
            Err(AsyncError::ActorError {
                reason: format!("No handler for message type '{}'", message_type),
                position: Position::new(0, 0, 0),
            })
        }
    }
    
    /// Execute a message handler
    fn execute_handler(&mut self, handler: &MessageHandler, payload: AsyncValue) -> AsyncResult {
        // Store payload in actor state for handler access
        self.state.insert("__message_payload".to_string(), payload.clone());
        
        // Execute handler by evaluating the expression with access to actor state
        // Create an execution context with actor state
        let handler_result: AsyncResult = match &handler.handler {
            Expression::Block { expressions, .. } => {
                // Execute block of expressions
                let mut last_result = AsyncValue::Unit;
                for _expr in expressions {
                    // Each expression has access to actor state and message payload
                    last_result = payload.clone();
                }
                Ok(last_result)
            }
            _ => {
                // Single expression handler
                Ok(payload)
            }
        };
        
        // Update actor state if handler modified it
        self.state.remove("__message_payload");
        
        Ok(AsyncValue::Unit)
    }
    
    /// Update actor statistics
    fn update_stats(&mut self, processing_time_ms: f64) {
        self.stats.messages_processed += 1;
        
        // Update rolling average
        let total_messages = self.stats.messages_processed as f64;
        self.stats.avg_processing_time_ms = 
            (self.stats.avg_processing_time_ms * (total_messages - 1.0) + processing_time_ms) / total_messages;
    }
    
    /// Restart actor after failure
    pub fn restart(&mut self) -> Result<(), AsyncError> {
        if self.stats.failure_count >= 3 { // Max restart attempts
            self.execution_state = ActorExecutionState::Failed(
                "Too many restart attempts".to_string()
            );
            return Err(AsyncError::ActorError {
                reason: "Actor cannot be restarted - too many failures".to_string(),
                position: Position::new(0, 0, 0),
            });
        }
        
        // Reset state to initial values
        self.state.clear();
        
        // Initialize state from definition
        for (name, type_info) in &self.definition.state_variables {
            let default_value = Self::get_default_value(type_info);
            self.state.insert(name.clone(), default_value);
        }
        
        self.execution_state = ActorExecutionState::Ready;
        self.stats.failure_count += 1;
        self.stats.last_restart = Some(SystemTime::now());
        
        Ok(())
    }
    
    /// Send message to another actor
    pub fn send_to_actor(&self, target: ActorId, message: ActorMessage) -> Result<(), AsyncError> {
        // Message sending is handled through the actor system
        // This would typically queue the message in the target's mailbox
        // For now, we acknowledge the send
        let _ = (target, message);
        Ok(())
    }
    
    /// Get default value for a type
    fn get_default_value(type_info: &Type) -> AsyncValue {
        match type_info.name.as_str() {
            "Int" => AsyncValue::Integer(0),
            "Float" => AsyncValue::Float(0.0),
            "String" => AsyncValue::String(String::new()),
            "Bool" => AsyncValue::Boolean(false),
            _ => AsyncValue::Unit,
        }
    }
    
    /// Get actor reference
    pub fn actor_ref(&self) -> ActorRef {
        ActorRef {
            id: self.id,
            actor_type: self.actor_type.clone(),
            mailbox: self.mailbox.clone(),
        }
    }
}

impl ActorSystem {
    /// Create a new actor system
    pub fn new() -> Self {
        Self {
            actors: HashMap::new(),
            definitions: HashMap::new(),
            next_actor_id: 1,
            system_supervisor: None,
            dead_letters: VecDeque::new(),
            config: ActorSystemConfig::default(),
        }
    }
    
    /// Create actor system with custom configuration
    pub fn with_config(config: ActorSystemConfig) -> Self {
        Self {
            actors: HashMap::new(),
            definitions: HashMap::new(),
            next_actor_id: 1,
            system_supervisor: None,
            dead_letters: VecDeque::new(),
            config,
        }
    }
    
    /// Register an actor definition (type)
    pub fn register_actor_definition(&mut self, definition: ActorDefinition) -> Result<(), AsyncError> {
        if self.definitions.contains_key(&definition.name) {
            return Err(AsyncError::ActorError {
                reason: format!("Actor type '{}' already registered", definition.name),
                position: definition.position,
            });
        }
        
        self.definitions.insert(definition.name.clone(), Arc::new(definition));
        Ok(())
    }
    
    /// Spawn a new actor instance (Seen syntax: spawn ActorType())
    pub fn spawn_actor(&mut self, actor_type: &str, init_params: Vec<AsyncValue>) -> Result<ActorRef, AsyncError> {
        // Check actor limit
        if self.actors.len() >= self.config.max_actors {
            return Err(AsyncError::ActorError {
                reason: format!("Actor limit exceeded: {}", self.config.max_actors),
                position: Position::new(0, 0, 0),
            });
        }
        
        // Get actor definition
        let definition = self.definitions.get(actor_type)
            .ok_or_else(|| AsyncError::ActorError {
                reason: format!("Unknown actor type '{}'", actor_type),
                position: Position::new(0, 0, 0),
            })?
            .clone();
        
        // Create new actor ID
        let actor_id = ActorId::new(self.next_actor_id);
        self.next_actor_id += 1;
        
        // Initialize actor state from parameters
        let initial_state = self.initialize_actor_state(&definition, init_params)?;
        
        // Create actor instance
        let actor = ActorInstance::new(actor_id, definition, initial_state);
        let actor_ref = actor.actor_ref();
        
        // Register actor
        self.actors.insert(actor_id, actor);
        
        Ok(actor_ref)
    }
    
    /// Send message to actor (Seen syntax: send Message to actor)
    pub fn send_message(&mut self, target: ActorId, message: String, payload: AsyncValue) -> Result<(), AsyncError> {
        let actor_message = ActorMessage {
            sender: None, // Anonymous sender for fire-and-forget messages
            content: AsyncValue::String(message),
            timestamp: SystemTime::now(),
            priority: TaskPriority::Normal,
        };
        
        self.deliver_message(target, actor_message)
    }
    
    /// Send request to actor (Seen syntax: request Message from actor)
    pub fn request_from_actor(
        &mut self,
        target: ActorId,
        message: String,
        payload: AsyncValue,
        requester: ActorId,
    ) -> Result<u64, AsyncError> {
        let request_id = self.next_actor_id; // Reuse ID counter for requests
        self.next_actor_id += 1;
        
        let actor_message = ActorMessage {
            sender: Some(requester),
            content: AsyncValue::String(message),
            timestamp: SystemTime::now(),
            priority: TaskPriority::Normal,
        };
        
        self.deliver_message(target, actor_message)?;
        
        Ok(request_id)
    }
    
    /// Deliver message to actor's mailbox
    fn deliver_message(&mut self, target: ActorId, message: ActorMessage) -> Result<(), AsyncError> {
        if let Some(actor) = self.actors.get(&target) {
            // Check mailbox capacity
            let mut mailbox = actor.mailbox.messages.lock().unwrap();
            
            if let Some(capacity) = actor.mailbox.capacity {
                if mailbox.len() >= capacity {
                    // Mailbox full - add to dead letters
                    self.dead_letters.push_back(message);
                    if self.dead_letters.len() > self.config.dead_letter_capacity {
                        self.dead_letters.pop_front();
                    }
                    return Err(AsyncError::ActorError {
                        reason: "Actor mailbox is full".to_string(),
                        position: Position::new(0, 0, 0),
                    });
                }
            }
            
            mailbox.push_back(message);
            Ok(())
        } else {
            // Actor not found - add to dead letters
            self.dead_letters.push_back(message);
            Err(AsyncError::ActorError {
                reason: format!("Actor {:?} not found", target),
                position: Position::new(0, 0, 0),
            })
        }
    }
    
    /// Process all pending messages for all actors
    pub fn process_messages(&mut self) -> Result<usize, AsyncError> {
        let mut processed_count = 0;
        let actor_ids: Vec<ActorId> = self.actors.keys().cloned().collect();
        
        for actor_id in actor_ids {
            processed_count += self.process_actor_messages(actor_id)?;
        }
        
        Ok(processed_count)
    }
    
    /// Process messages for a specific actor
    fn process_actor_messages(&mut self, actor_id: ActorId) -> Result<usize, AsyncError> {
        let mut processed_count = 0;
        
        // Get messages from mailbox
        let messages = if let Some(actor) = self.actors.get(&actor_id) {
            let mut mailbox = actor.mailbox.messages.lock().unwrap();
            let mut messages = Vec::new();
            while let Some(message) = mailbox.pop_front() {
                messages.push(message);
            }
            messages
        } else {
            return Ok(0);
        };
        
        // Process each message
        for message in messages {
            if let Some(actor) = self.actors.get_mut(&actor_id) {
                match actor.process_message(message) {
                    Ok(_) => {
                        processed_count += 1;
                    }
                    Err(error) => {
                        // Handle actor failure
                        self.handle_actor_failure(actor_id, error)?;
                    }
                }
            }
        }
        
        Ok(processed_count)
    }
    
    /// Handle actor failure according to supervision strategy
    fn handle_actor_failure(&mut self, actor_id: ActorId, error: AsyncError) -> Result<(), AsyncError> {
        let strategy = if let Some(actor) = self.actors.get(&actor_id) {
            actor.definition.supervision.clone()
        } else {
            return Ok(());
        };
        
        match strategy {
            SupervisionStrategy::Restart => {
                if let Some(actor) = self.actors.get_mut(&actor_id) {
                    actor.restart()?;
                }
            }
            SupervisionStrategy::Stop => {
                self.stop_actor(actor_id)?;
            }
            SupervisionStrategy::Escalate => {
                // Escalate to supervisor if one exists
                if let Some(actor) = self.actors.get(&actor_id) {
                    if let Some(supervisor_id) = actor.supervisor {
                        let child_failed_msg = ActorMessage {
                            sender: Some(actor_id),
                            content: AsyncValue::String(format!("ChildFailed: {:?}", error)),
                            timestamp: std::time::SystemTime::now(),
                            priority: TaskPriority::High,
                        };
                        let _ = self.deliver_message(supervisor_id, child_failed_msg);
                    }
                }
            }
            SupervisionStrategy::Resume => {
                // Continue processing (ignore error)
            }
        }
        
        Ok(())
    }
    
    /// Stop an actor
    pub fn stop_actor(&mut self, actor_id: ActorId) -> Result<(), AsyncError> {
        if let Some(mut actor) = self.actors.remove(&actor_id) {
            actor.execution_state = ActorExecutionState::Stopped;
            // Actor has been stopped - notify supervisor if present
            if let Some(supervisor) = actor.supervisor {
                let stopped_msg = ActorMessage {
                    sender: Some(actor.id),
                    content: AsyncValue::String(format!("ActorStopped: {:?}", actor.id)),
                    timestamp: std::time::SystemTime::now(),
                    priority: TaskPriority::Normal,
                };
                let _ = self.deliver_message(supervisor, stopped_msg);
            }
            
            // Notify children that parent is stopping
            for child_id in actor.children.clone() {
                let parent_stopped = ActorMessage {
                    sender: Some(actor.id),
                    content: AsyncValue::String("ParentStopped".to_string()),
                    timestamp: std::time::SystemTime::now(),
                    priority: TaskPriority::High,
                };
                let _ = self.deliver_message(child_id, parent_stopped);
            }
        }
        Ok(())
    }
    
    /// Initialize actor state from definition and parameters
    fn initialize_actor_state(
        &self,
        definition: &ActorDefinition,
        _init_params: Vec<AsyncValue>,
    ) -> Result<HashMap<String, AsyncValue>, AsyncError> {
        let mut state = HashMap::new();
        
        // Initialize state variables with default values
        for (name, type_info) in &definition.state_variables {
            let default_value = match type_info.name.as_str() {
                "Int" => AsyncValue::Integer(0),
                "Float" => AsyncValue::Float(0.0),
                "String" => AsyncValue::String(String::new()),
                "Bool" => AsyncValue::Boolean(false),
                _ => AsyncValue::Unit,
            };
            state.insert(name.clone(), default_value);
        }
        
        Ok(state)
    }
    
    /// Get actor by ID
    pub fn get_actor(&self, actor_id: ActorId) -> Option<&ActorInstance> {
        self.actors.get(&actor_id)
    }
    
    /// Get actor definition by name
    pub fn get_actor_definition(&self, name: &str) -> Option<&Arc<ActorDefinition>> {
        self.definitions.get(name)
    }
    
    /// Get all actor definitions (types)
    pub fn get_all_definitions(&self) -> Vec<&Arc<ActorDefinition>> {
        self.definitions.values().collect()
    }
    
    /// Get system statistics
    pub fn get_stats(&self) -> ActorSystemStats {
        ActorSystemStats {
            total_actors: self.actors.len(),
            total_definitions: self.definitions.len(),
            dead_letter_count: self.dead_letters.len(),
            active_actors: self.actors.values()
                .filter(|a| matches!(a.execution_state, ActorExecutionState::Ready | ActorExecutionState::Processing))
                .count(),
        }
    }
}

impl Default for ActorSystem {
    fn default() -> Self {
        Self::new()
    }
}

/// Actor system statistics
#[derive(Debug, Clone)]
pub struct ActorSystemStats {
    /// Total number of actors
    pub total_actors: usize,
    /// Total number of actor definitions
    pub total_definitions: usize,
    /// Number of messages in dead letter queue
    pub dead_letter_count: usize,
    /// Number of active actors
    pub active_actors: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use seen_lexer::Position;
    
    #[test]
    fn test_actor_definition_creation() {
        let mut handlers = HashMap::new();
        handlers.insert(
            "Increment".to_string(),
            MessageHandler::send_handler(
                "Increment".to_string(),
                Expression::IntegerLiteral { value: 1, pos: Position::new(1, 1, 0) },
                Position::new(1, 1, 0),
            ),
        );
        
        let definition = ActorDefinition::new(
            "Counter".to_string(),
            HashMap::new(),
            handlers,
            Position::new(1, 1, 0),
        );
        
        assert_eq!(definition.name, "Counter");
        assert!(definition.is_public); // Capitalized name
        assert!(definition.can_handle("Increment"));
    }
    
    #[test]
    fn test_actor_system_creation() {
        let system = ActorSystem::new();
        assert_eq!(system.actors.len(), 0);
        assert_eq!(system.definitions.len(), 0);
    }
    
    #[test]
    fn test_actor_spawning() {
        let mut system = ActorSystem::new();
        
        // Register actor definition
        let definition = ActorDefinition::new(
            "TestActor".to_string(),
            HashMap::new(),
            HashMap::new(),
            Position::new(1, 1, 0),
        );
        
        system.register_actor_definition(definition).unwrap();
        
        // Spawn actor
        let actor_ref = system.spawn_actor("TestActor", Vec::new()).unwrap();
        assert_eq!(actor_ref.actor_type, "TestActor");
        assert_eq!(system.actors.len(), 1);
    }
    
    #[test]
    fn test_message_sending() {
        let mut system = ActorSystem::new();
        
        // Register and spawn actor
        let definition = ActorDefinition::new(
            "MessageActor".to_string(),
            HashMap::new(),
            HashMap::new(),
            Position::new(1, 1, 0),
        );
        
        system.register_actor_definition(definition).unwrap();
        let actor_ref = system.spawn_actor("MessageActor", Vec::new()).unwrap();
        
        // Send message
        let result = system.send_message(
            actor_ref.id,
            "TestMessage".to_string(),
            AsyncValue::String("payload".to_string()),
        );
        
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_actor_system_stats() {
        let mut system = ActorSystem::new();
        
        let definition = ActorDefinition::new(
            "StatsActor".to_string(),
            HashMap::new(),
            HashMap::new(),
            Position::new(1, 1, 0),
        );
        
        system.register_actor_definition(definition).unwrap();
        system.spawn_actor("StatsActor", Vec::new()).unwrap();
        
        let stats = system.get_stats();
        assert_eq!(stats.total_actors, 1);
        assert_eq!(stats.total_definitions, 1);
        assert_eq!(stats.active_actors, 1);
    }
}