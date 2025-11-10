//! Runtime tracing support for the Seen interpreter.

use crate::value::Value;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

/// Version number for the runtime trace schema.
const TRACE_VERSION: u32 = 1;

/// Serializable trace artifact written by `seen trace --runtime`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeTraceFile {
    pub version: u32,
    pub captured_at_epoch_ms: u128,
    pub metadata: RuntimeTraceMetadata,
    pub events: Vec<RuntimeTraceEvent>,
}

/// Static metadata describing the captured program/configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeTraceMetadata {
    pub program: String,
    pub opt_level: u8,
    pub cli_profile: String,
    pub args: Vec<String>,
    pub seen_version: String,
    pub host: String,
}

/// Handle shared between the interpreter and CLI to accumulate events.
#[derive(Clone)]
pub struct RuntimeTraceHandle {
    inner: Arc<Mutex<RuntimeTraceCollector>>,
}

impl RuntimeTraceHandle {
    /// Create a new trace handle with the given metadata.
    pub fn new(metadata: RuntimeTraceMetadata) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let collector = RuntimeTraceCollector {
            version: TRACE_VERSION,
            captured_at_epoch_ms: timestamp,
            metadata,
            events: Vec::new(),
        };
        Self {
            inner: Arc::new(Mutex::new(collector)),
        }
    }

    /// Record a new event if the handle is still alive.
    pub fn record(&self, event: RuntimeTraceEvent) {
        if let Ok(mut guard) = self.inner.lock() {
            guard.events.push(event);
        }
    }

    /// Convert the handle into a serializable trace file.
    pub fn into_trace_file(self) -> RuntimeTraceFile {
        match Arc::try_unwrap(self.inner) {
            Ok(mutex) => mutex
                .into_inner()
                .map(|collector| collector.into_trace_file())
                .unwrap_or_else(|poisoned| poisoned.into_inner().into_trace_file()),
            Err(shared) => shared
                .lock()
                .map(|collector| collector.clone().into_trace_file())
                .unwrap_or_else(|poisoned| poisoned.into_inner().clone().into_trace_file()),
        }
    }

    /// Clone the current trace contents without consuming the handle.
    pub fn snapshot(&self) -> RuntimeTraceFile {
        self.inner
            .lock()
            .map(|collector| collector.clone().into_trace_file())
            .unwrap_or_else(|poisoned| poisoned.into_inner().clone().into_trace_file())
    }
}

#[derive(Debug, Clone)]
struct RuntimeTraceCollector {
    version: u32,
    captured_at_epoch_ms: u128,
    metadata: RuntimeTraceMetadata,
    events: Vec<RuntimeTraceEvent>,
}

impl RuntimeTraceCollector {
    fn into_trace_file(self) -> RuntimeTraceFile {
        RuntimeTraceFile {
            version: self.version,
            captured_at_epoch_ms: self.captured_at_epoch_ms,
            metadata: self.metadata,
            events: self.events,
        }
    }
}

/// Events captured while the interpreter executes a program.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RuntimeTraceEvent {
    ProgramStart {
        expression_count: usize,
    },
    ProgramEnd {
        result: RuntimeTraceValue,
    },
    ProgramError {
        message: String,
    },
    FunctionEnter {
        name: String,
        arguments: Vec<RuntimeTraceValue>,
    },
    FunctionExit {
        name: String,
        result: RuntimeTraceValue,
    },
    FunctionError {
        name: String,
        message: String,
    },
    MethodEnter {
        class: String,
        method: String,
        arguments: Vec<RuntimeTraceValue>,
    },
    MethodExit {
        class: String,
        method: String,
        result: RuntimeTraceValue,
    },
    MethodError {
        class: String,
        method: String,
        message: String,
    },
    EffectRegistered {
        effect: String,
        effect_id: String,
        operations: Vec<String>,
    },
    EffectHandleEnter {
        effect: String,
        effect_id: String,
        handlers: Vec<String>,
    },
    EffectHandleExit {
        effect: String,
        effect_id: String,
        result: Option<RuntimeTraceValue>,
        error: Option<String>,
    },
    EffectOperationInvoke {
        effect: String,
        operation: String,
        arguments: Vec<RuntimeTraceValue>,
        via_handler: bool,
    },
    EffectOperationResult {
        effect: String,
        operation: String,
        result: RuntimeTraceValue,
    },
    EffectOperationError {
        effect: String,
        operation: String,
        message: String,
    },
    CrashBreadcrumb {
        phase: String,
        message: String,
    },
}

/// Lossy snapshot of interpreter values that is safe to serialize into the trace.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum RuntimeTraceValue {
    Unit,
    Null,
    Integer(i64),
    Float(f64),
    Boolean(bool),
    String(String),
    Character(char),
    Array { length: usize },
    Bytes { length: usize },
    Struct { name: String },
    Class { name: String },
    Function { name: String },
    Promise { state: String },
    Task,
    Channel,
    Actor,
    Effect { name: String },
    EffectHandle { effect_id: String },
    Observable,
    Flow,
    ReactiveProperty { name: String },
    Unknown { type_name: String },
}

impl RuntimeTraceValue {
    /// Construct a serializable snapshot from an interpreter value.
    pub fn from_value(value: &Value) -> Self {
        match value {
            Value::Unit => RuntimeTraceValue::Unit,
            Value::Null => RuntimeTraceValue::Null,
            Value::Integer(i) => RuntimeTraceValue::Integer(*i),
            Value::Float(f) => RuntimeTraceValue::Float(*f),
            Value::Boolean(b) => RuntimeTraceValue::Boolean(*b),
            Value::String(s) => RuntimeTraceValue::String(s.clone()),
            Value::Character(c) => RuntimeTraceValue::Character(*c),
            Value::Array(items) => {
                let length = items.lock().map(|vals| vals.len()).unwrap_or(0);
                RuntimeTraceValue::Array { length }
            }
            Value::Bytes(bytes) => RuntimeTraceValue::Bytes {
                length: bytes.len(),
            },
            Value::Struct { name, .. } => RuntimeTraceValue::Struct { name: name.clone() },
            Value::Class { name } => RuntimeTraceValue::Class { name: name.clone() },
            Value::Function { name, .. } => RuntimeTraceValue::Function { name: name.clone() },
            Value::Promise(promise) => {
                let state = if promise.is_pending() {
                    "pending"
                } else if promise.is_resolved() {
                    "resolved"
                } else if promise.is_rejected() {
                    "rejected"
                } else {
                    "unknown"
                };
                RuntimeTraceValue::Promise {
                    state: state.to_string(),
                }
            }
            Value::Task(_) => RuntimeTraceValue::Task,
            Value::Channel(_) => RuntimeTraceValue::Channel,
            Value::Actor(_) => RuntimeTraceValue::Actor,
            Value::Effect(effect) => RuntimeTraceValue::Effect {
                name: effect.name.clone(),
            },
            Value::EffectHandle { effect_id, .. } => RuntimeTraceValue::EffectHandle {
                effect_id: effect_id.id().to_string(),
            },
            Value::Observable(_) => RuntimeTraceValue::Observable,
            Value::Flow(_) => RuntimeTraceValue::Flow,
            Value::ReactiveProperty { name, .. } => {
                RuntimeTraceValue::ReactiveProperty { name: name.clone() }
            }
        }
    }
}
