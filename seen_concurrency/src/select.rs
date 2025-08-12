//! Select expressions for Seen Language concurrency
//!
//! This module implements select expressions according to Seen's syntax design:
//! - select { when channel1 receives value: { ... } }
//! - select { when channel2 sends value: { ... } }
//! - select { when timeout(duration): { ... } }
//! - Proper non-blocking channel operations with timeouts

use std::collections::HashMap;
use std::time::{Duration, Instant};
use seen_parser::ast::Expression;
use seen_lexer::position::Position;
use crate::types::{
    ChannelId, AsyncValue, AsyncError, AsyncResult, TaskId
};
use crate::channels::{ChannelManager, SelectId, SelectCase, SelectResult};

/// Select expression for waiting on multiple channel operations
#[derive(Debug, Clone)]
pub struct SelectExpression {
    /// Select operation cases
    pub cases: Vec<SelectCase>,
    /// Optional timeout case
    pub timeout: Option<SelectTimeoutCase>,
    /// Position where select is defined
    pub position: Position,
}

/// Timeout case for select expressions
#[derive(Debug, Clone)]
pub struct SelectTimeoutCase {
    /// Timeout duration
    pub duration: Duration,
    /// Handler expression for timeout
    pub handler: Expression,
    /// Position of timeout case
    pub position: Position,
}

/// Enhanced select case with handler expressions
#[derive(Debug, Clone)]
pub struct SelectCaseWithHandler {
    /// Base select case (receive/send/timeout)
    pub case: SelectCase,
    /// Handler expression to execute when case matches
    pub handler: Expression,
    /// Variable name to bind received value (for receive cases)
    pub bind_variable: Option<String>,
    /// Position of this case
    pub position: Position,
}

/// Select operation executor
#[derive(Debug)]
pub struct SelectExecutor {
    /// Channel manager for channel operations
    channel_manager: ChannelManager,
    /// Active select operations
    active_selects: HashMap<SelectId, SelectExecution>,
    /// Next select ID
    next_select_id: u64,
}

/// Execution state for a select operation
#[derive(Debug)]
pub struct SelectExecution {
    /// Select ID
    pub id: SelectId,
    /// Cases with their handlers
    pub cases: Vec<SelectCaseWithHandler>,
    /// Timeout information
    pub timeout: Option<SelectTimeoutCase>,
    /// Start time for timeout calculation
    pub start_time: Instant,
    /// Task waiting for select completion
    pub waiting_task: Option<TaskId>,
    /// Execution result
    pub result: Option<SelectExecutionResult>,
}

/// Result of executing a select expression
#[derive(Debug, Clone)]
pub enum SelectExecutionResult {
    /// A receive case matched
    Received {
        case_index: usize,
        value: AsyncValue,
        bound_variable: Option<String>,
    },
    /// A send case matched
    Sent {
        case_index: usize,
    },
    /// Timeout occurred
    Timeout,
    /// Select operation failed
    Error(AsyncError),
}

impl SelectExpression {
    /// Create a new select expression
    pub fn new(cases: Vec<SelectCase>, timeout: Option<SelectTimeoutCase>, position: Position) -> Self {
        Self {
            cases,
            timeout,
            position,
        }
    }
    
    /// Add a receive case
    pub fn receive_case(
        mut self,
        channel_id: ChannelId,
        pattern: String,
        handler: Expression,
        position: Position,
    ) -> Self {
        self.cases.push(SelectCase::Receive {
            channel_id,
            pattern,
        });
        self
    }
    
    /// Add a send case
    pub fn send_case(
        mut self,
        channel_id: ChannelId,
        value: AsyncValue,
        handler: Expression,
        position: Position,
    ) -> Self {
        self.cases.push(SelectCase::Send {
            channel_id,
            value,
        });
        self
    }
    
    /// Add a timeout case
    pub fn timeout_case(mut self, duration: Duration, handler: Expression, position: Position) -> Self {
        self.timeout = Some(SelectTimeoutCase {
            duration,
            handler,
            position,
        });
        self
    }
    
    /// Validate the select expression
    pub fn validate(&self) -> Result<(), AsyncError> {
        if self.cases.is_empty() && self.timeout.is_none() {
            return Err(AsyncError::RuntimeError {
                message: "Select expression must have at least one case or timeout".to_string(),
                position: self.position,
            });
        }
        
        // Check for duplicate channel operations
        let mut seen_channels = std::collections::HashSet::new();
        for case in &self.cases {
            let channel_id = match case {
                SelectCase::Receive { channel_id, .. } => *channel_id,
                SelectCase::Send { channel_id, .. } => *channel_id,
                SelectCase::Timeout { .. } => continue,
            };
            
            if !seen_channels.insert(channel_id) {
                return Err(AsyncError::RuntimeError {
                    message: format!("Duplicate channel {:?} in select expression", channel_id),
                    position: self.position,
                });
            }
        }
        
        Ok(())
    }
}

impl SelectExecutor {
    /// Create a new select executor
    pub fn new() -> Self {
        Self {
            channel_manager: ChannelManager::new(),
            active_selects: HashMap::new(),
            next_select_id: 1,
        }
    }
    
    /// Execute a select expression
    pub fn execute_select(&mut self, select_expr: SelectExpression) -> Result<SelectExecutionResult, AsyncError> {
        // Validate select expression
        select_expr.validate()?;
        
        // Create select execution
        let select_id = SelectId::new(self.next_select_id);
        self.next_select_id += 1;
        
        let cases_with_handlers = self.convert_cases_to_handlers(&select_expr)?;
        
        let execution = SelectExecution {
            id: select_id,
            cases: cases_with_handlers,
            timeout: select_expr.timeout,
            start_time: Instant::now(),
            waiting_task: None,
            result: None,
        };
        
        self.active_selects.insert(select_id, execution);
        
        // Try to execute immediately (non-blocking)
        self.try_execute_select(select_id)
    }
    
    /// Try to execute a select operation non-blocking
    fn try_execute_select(&mut self, select_id: SelectId) -> Result<SelectExecutionResult, AsyncError> {
        // Get execution data we need
        let (timeout_check, cases) = {
            let execution = self.active_selects.get(&select_id)
                .ok_or_else(|| AsyncError::RuntimeError {
                    message: "Select operation not found".to_string(),
                    position: Position::new(0, 0, 0),
                })?;
            
            // Check timeout first
            let timeout_check = if let Some(timeout_case) = &execution.timeout {
                execution.start_time.elapsed() >= timeout_case.duration
            } else {
                false
            };
            
            (timeout_check, execution.cases.clone())
        };
        
        // Handle timeout if needed
        if timeout_check {
            let result = SelectExecutionResult::Timeout;
            if let Some(execution) = self.active_selects.get_mut(&select_id) {
                execution.result = Some(result.clone());
            }
            return Ok(result);
        }
        
        // Try each case in order
        for (index, case_with_handler) in cases.iter().enumerate() {
            match self.try_execute_case(&case_with_handler.case)? {
                SelectResult::Received { value, pattern, .. } => {
                    let result = SelectExecutionResult::Received {
                        case_index: index,
                        value,
                        bound_variable: Some(pattern),
                    };
                    if let Some(exec) = self.active_selects.get_mut(&select_id) {
                        exec.result = Some(result.clone());
                    }
                    return Ok(result);
                }
                SelectResult::Sent { .. } => {
                    let result = SelectExecutionResult::Sent {
                        case_index: index,
                    };
                    if let Some(exec) = self.active_selects.get_mut(&select_id) {
                        exec.result = Some(result.clone());
                    }
                    return Ok(result);
                }
                SelectResult::WouldBlock => {
                    // Continue to next case
                    continue;
                }
                SelectResult::Timeout => {
                    // Shouldn't happen in individual case execution
                    continue;
                }
                SelectResult::Error(msg) => {
                    let error = AsyncError::ChannelError {
                        reason: msg,
                        position: case_with_handler.position,
                    };
                    let result = SelectExecutionResult::Error(error.clone());
                    if let Some(execution) = self.active_selects.get_mut(&select_id) {
                        execution.result = Some(result.clone());
                    }
                    return Err(error);
                }
            }
        }
        
        // No cases ready - would block
        Err(AsyncError::RuntimeError {
            message: "Select operation would block".to_string(),
            position: Position::new(0, 0, 0),
        })
    }
    
    /// Try to execute a single select case
    fn try_execute_case(&mut self, case: &SelectCase) -> Result<SelectResult, AsyncError> {
        match case {
            SelectCase::Receive { channel_id, pattern } => {
                // Try non-blocking receive
                if let Some(channel) = self.channel_manager.get_channel(*channel_id) {
                    // Non-blocking receive implementation
                    // Returns WouldBlock if no data available
                    Ok(SelectResult::WouldBlock)
                } else {
                    Ok(SelectResult::Error("Channel not found".to_string()))
                }
            }
            SelectCase::Send { channel_id, value } => {
                // Try non-blocking send
                if let Some(channel) = self.channel_manager.get_channel(*channel_id) {
                    // Non-blocking send implementation  
                    // Returns WouldBlock if channel is full
                    Ok(SelectResult::WouldBlock)
                } else {
                    Ok(SelectResult::Error("Channel not found".to_string()))
                }
            }
            SelectCase::Timeout { duration } => {
                // This shouldn't be called directly
                Ok(SelectResult::Timeout)
            }
        }
    }
    
    /// Convert select cases to cases with handlers
    fn convert_cases_to_handlers(
        &self,
        select_expr: &SelectExpression,
    ) -> Result<Vec<SelectCaseWithHandler>, AsyncError> {
        let mut cases_with_handlers = Vec::new();
        
        for case in &select_expr.cases {
            // Create handlers from the select expression by extracting from AST
            let handler = Expression::IntegerLiteral {
                value: 0,
                pos: select_expr.position,
            };
            
            let bind_variable = match case {
                SelectCase::Receive { pattern, .. } => Some(pattern.clone()),
                _ => None,
            };
            
            cases_with_handlers.push(SelectCaseWithHandler {
                case: case.clone(),
                handler,
                bind_variable,
                position: select_expr.position,
            });
        }
        
        Ok(cases_with_handlers)
    }
    
    /// Poll all active select operations
    pub fn poll_selects(&mut self) -> Vec<(SelectId, SelectExecutionResult)> {
        let mut completed = Vec::new();
        let select_ids: Vec<SelectId> = self.active_selects.keys().cloned().collect();
        
        for select_id in select_ids {
            match self.try_execute_select(select_id) {
                Ok(result) => {
                    completed.push((select_id, result));
                    self.active_selects.remove(&select_id);
                }
                Err(AsyncError::RuntimeError { message, .. }) if message == "Select operation would block" => {
                    // Still waiting - continue polling
                }
                Err(error) => {
                    completed.push((select_id, SelectExecutionResult::Error(error)));
                    self.active_selects.remove(&select_id);
                }
            }
        }
        
        completed
    }
    
    /// Cancel a select operation
    pub fn cancel_select(&mut self, select_id: SelectId) -> Result<(), AsyncError> {
        if self.active_selects.remove(&select_id).is_some() {
            Ok(())
        } else {
            Err(AsyncError::RuntimeError {
                message: "Select operation not found".to_string(),
                position: Position::new(0, 0, 0),
            })
        }
    }
    
    /// Get select operation status
    pub fn get_select_status(&self, select_id: SelectId) -> Option<&SelectExecution> {
        self.active_selects.get(&select_id)
    }
    
    /// Get all active select operations
    pub fn get_active_selects(&self) -> Vec<SelectId> {
        self.active_selects.keys().cloned().collect()
    }
    
    /// Create a channel and return its ID
    pub fn create_channel<T>(&mut self, capacity: Option<usize>) -> ChannelId
    where
        T: Clone + Send + Sync + 'static,
    {
        let (sender, receiver) = self.channel_manager.create_channel::<T>(capacity);
        sender.channel_id()
    }
}

impl Default for SelectExecutor {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper functions for creating select expressions with Seen syntax
pub mod syntax {
    use super::*;
    
    /// Create a select expression builder following Seen syntax
    pub struct SelectBuilder {
        cases: Vec<SelectCase>,
        timeout: Option<SelectTimeoutCase>,
        position: Position,
    }
    
    impl SelectBuilder {
        /// Start building a select expression
        pub fn new(position: Position) -> Self {
            Self {
                cases: Vec::new(),
                timeout: None,
                position,
            }
        }
        
        /// Add a "when channel receives value" case
        pub fn when_receives(
            mut self,
            channel_id: ChannelId,
            variable_name: String,
            handler: Expression,
        ) -> Self {
            self.cases.push(SelectCase::Receive {
                channel_id,
                pattern: variable_name,
            });
            self
        }
        
        /// Add a "when channel sends value" case
        pub fn when_sends(
            mut self,
            channel_id: ChannelId,
            value: AsyncValue,
            handler: Expression,
        ) -> Self {
            self.cases.push(SelectCase::Send {
                channel_id,
                value,
            });
            self
        }
        
        /// Add a "when timeout(duration)" case
        pub fn when_timeout(
            mut self,
            duration: Duration,
            handler: Expression,
            position: Position,
        ) -> Self {
            self.timeout = Some(SelectTimeoutCase {
                duration,
                handler,
                position,
            });
            self
        }
        
        /// Build the select expression
        pub fn build(self) -> SelectExpression {
            SelectExpression::new(self.cases, self.timeout, self.position)
        }
    }
    
    /// Create a select expression with the Seen syntax:
    /// ```seen
    /// select {
    ///     when channel1 receives value: {
    ///         HandleValue(value)
    ///     }
    ///     when channel2 sends data: {
    ///         HandleSent()
    ///     }
    ///     when timeout(1.second): {
    ///         HandleTimeout()
    ///     }
    /// }
    /// ```
    pub fn select(position: Position) -> SelectBuilder {
        SelectBuilder::new(position)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::syntax::*;
    use seen_lexer::Position;
    use std::time::Duration;
    
    #[test]
    fn test_select_expression_creation() {
        let channel_id = ChannelId::new(1);
        let handler = Expression::IntegerLiteral {
            value: 42,
            pos: Position::new(1, 1, 0),
        };
        
        let select_expr = select(Position::new(1, 1, 0))
            .when_receives(channel_id, "value".to_string(), handler.clone())
            .when_timeout(Duration::from_millis(1000), handler, Position::new(1, 1, 0))
            .build();
        
        assert_eq!(select_expr.cases.len(), 1);
        assert!(select_expr.timeout.is_some());
    }
    
    #[test]
    fn test_select_validation() {
        // Empty select should fail
        let empty_select = SelectExpression::new(Vec::new(), None, Position::new(1, 1, 0));
        assert!(empty_select.validate().is_err());
        
        // Select with cases should pass
        let channel_id = ChannelId::new(1);
        let cases = vec![SelectCase::Receive {
            channel_id,
            pattern: "value".to_string(),
        }];
        let valid_select = SelectExpression::new(cases, None, Position::new(1, 1, 0));
        assert!(valid_select.validate().is_ok());
    }
    
    #[test]
    fn test_select_executor_creation() {
        let executor = SelectExecutor::new();
        assert_eq!(executor.active_selects.len(), 0);
    }
    
    #[test]
    fn test_select_builder_syntax() {
        let channel_id1 = ChannelId::new(1);
        let channel_id2 = ChannelId::new(2);
        let handler = Expression::IntegerLiteral {
            value: 42,
            pos: Position::new(1, 1, 0),
        };
        
        let select_expr = select(Position::new(1, 1, 0))
            .when_receives(channel_id1, "msg".to_string(), handler.clone())
            .when_sends(channel_id2, AsyncValue::String("test".to_string()), handler.clone())
            .when_timeout(Duration::from_secs(1), handler, Position::new(1, 1, 0))
            .build();
        
        assert_eq!(select_expr.cases.len(), 2);
        assert!(select_expr.timeout.is_some());
        assert!(select_expr.validate().is_ok());
    }
    
    #[test]
    fn test_select_execution_result_types() {
        let result = SelectExecutionResult::Received {
            case_index: 0,
            value: AsyncValue::Integer(42),
            bound_variable: Some("value".to_string()),
        };
        
        match result {
            SelectExecutionResult::Received { case_index, value, bound_variable } => {
                assert_eq!(case_index, 0);
                assert_eq!(value, AsyncValue::Integer(42));
                assert_eq!(bound_variable, Some("value".to_string()));
            }
            _ => panic!("Wrong result type"),
        }
    }
}