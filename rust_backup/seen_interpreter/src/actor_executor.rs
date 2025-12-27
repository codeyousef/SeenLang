use crate::interpreter::Interpreter;
use crate::runtime::Environment;
use crate::value_bridge::{async_to_value, value_to_async};
use seen_concurrency::actors::{ActorHandlerExecutor, ActorInstance, MessageHandler};
use seen_concurrency::types::{AsyncError, AsyncResult, AsyncValue};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// Bridges actor handler execution back into the interpreter.
pub struct InterpreterActorExecutor {
    contexts: Mutex<HashMap<u64, Environment>>,
    next_id: AtomicU64,
}

impl InterpreterActorExecutor {
    pub fn new() -> Self {
        Self {
            contexts: Mutex::new(HashMap::new()),
            next_id: AtomicU64::new(1),
        }
    }

    /// Store an environment snapshot for later use and return its identifier.
    pub fn register_context(&self, env: Environment) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        if let Ok(mut ctx) = self.contexts.lock() {
            ctx.insert(id, env);
        }
        id
    }

    fn load_context(&self, id: u64) -> Option<Environment> {
        self.contexts
            .lock()
            .ok()
            .and_then(|map| map.get(&id).cloned())
    }
}

impl ActorHandlerExecutor for InterpreterActorExecutor {
    fn execute(
        &self,
        actor: &mut ActorInstance,
        handler: &MessageHandler,
        payload: AsyncValue,
    ) -> AsyncResult {
        let mut interpreter = Interpreter::new();

        if let Some(context_id) = handler.executor_context_id {
            if let Some(env) = self.load_context(context_id) {
                interpreter.runtime_mut().initialize_with_environment(env);
            }
        }

        let mut field_values = HashMap::new();
        for (name, value) in actor.state.iter() {
            field_values.insert(name.clone(), async_to_value(value));
        }
        let field_arc = Arc::new(Mutex::new(field_values));
        interpreter.push_instance_context(actor.definition.name.clone(), Arc::clone(&field_arc));

        {
            let runtime = interpreter.runtime_mut();
            runtime.define_variable("__message_payload".to_string(), async_to_value(&payload));
        }

        let result = interpreter
            .interpret_expression(&handler.handler)
            .map_err(|err| AsyncError::ActorError {
                reason: format!("Actor handler failed: {:?}", err),
                position: handler.position,
            })?;

        interpreter.pop_instance_context();

        if let Ok(fields) = field_arc.lock() {
            for (field_name, value) in fields.iter() {
                actor
                    .state
                    .insert(field_name.clone(), value_to_async(value));
            }
        }

        Ok(value_to_async(&result))
    }
}
