use crate::value::Value;
use seen_concurrency::types::AsyncValue;
use std::sync::Arc;

/// Convert an async runtime value into an interpreter value.
pub fn async_to_value(async_value: &AsyncValue) -> Value {
    match async_value {
        AsyncValue::Unit => Value::Unit,
        AsyncValue::Integer(i) => Value::Integer(*i),
        AsyncValue::Float(f) => Value::Float(*f),
        AsyncValue::Boolean(b) => Value::Boolean(*b),
        AsyncValue::String(s) => Value::String(s.clone()),
        AsyncValue::Array(values) => {
            let converted: Vec<Value> = values.iter().map(async_to_value).collect();
            Value::array_from_vec(converted)
        }
        AsyncValue::Promise(promise) => Value::Promise(Arc::clone(promise)),
        AsyncValue::Channel(channel) => Value::Channel(channel.clone()),
        AsyncValue::Actor(actor) => Value::Actor(actor.clone()),
        AsyncValue::Error => Value::Null,
        AsyncValue::Pending => Value::Unit,
    }
}

/// Convert an interpreter value into an async runtime value.
pub fn value_to_async(value: &Value) -> AsyncValue {
    match value {
        Value::Unit => AsyncValue::Unit,
        Value::Integer(i) => AsyncValue::Integer(*i),
        Value::Float(f) => AsyncValue::Float(*f),
        Value::Boolean(b) => AsyncValue::Boolean(*b),
        Value::String(s) => AsyncValue::String(s.clone()),
        Value::Array(values) => {
            let guard = values
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let converted: Vec<AsyncValue> = guard.iter().map(value_to_async).collect();
            AsyncValue::Array(converted)
        }
        Value::Promise(promise) => AsyncValue::Promise(promise.clone()),
        Value::Channel(channel) => AsyncValue::Channel(channel.clone()),
        Value::Actor(actor) => AsyncValue::Actor(actor.clone()),
        _ => AsyncValue::Unit,
    }
}
