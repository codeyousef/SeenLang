use crate::{
    function::LocalVariable,
    instruction::Label,
    value::{IRType, IRValue},
};
use indexmap::IndexMap;

// Deterministic maps to keep codegen stable
pub(crate) type HashMap<K, V> = IndexMap<K, V>;

/// Context for IR generation
#[derive(Debug)]
pub struct GenerationContext {
    pub(crate) current_function: Option<String>,
    pub(crate) current_block: Option<String>,
    pub(crate) variable_types: HashMap<String, IRType>,
    pub(crate) register_types: HashMap<u32, IRType>, // Track types of registers
    pub(crate) local_variables: Vec<LocalVariable>, // Track local variables for current function
    pub(crate) register_counter: u32,
    pub(crate) label_counter: u32,
    pub(crate) break_stack: Vec<String>,    // Labels for break statements
    pub(crate) continue_stack: Vec<String>, // Labels for continue statements
    pub(crate) string_table: HashMap<String, u32>, // String interning table
    pub(crate) type_definitions: HashMap<String, IRType>, // Registered type definitions (structs/classes/enums)
    pub(crate) function_return_types: HashMap<String, IRType>, // Function name -> return type
    pub(crate) _current_receiver_type: Option<IRType>, // Type of 'this' in current method context
    pub(crate) _current_receiver_name: Option<String>, // Name of the receiver parameter (e.g., "self", "this")
    pub(crate) current_type_definition: Option<String>, // Name of type currently being defined
    pub(crate) result_inner_types: HashMap<String, IRType>, // Track inner type T for Result<T, E> variables
    pub(crate) container_element_types: HashMap<String, IRType>, // Track element type T for Vec<T>, Option<T>, etc.
}

impl GenerationContext {
    pub fn new() -> Self {
        Self {
            current_function: None,
            current_block: None,
            variable_types: HashMap::new(),
            register_types: HashMap::new(),
            local_variables: Vec::new(),
            register_counter: 0,
            label_counter: 0,
            break_stack: Vec::new(),
            continue_stack: Vec::new(),
            string_table: HashMap::new(),
            type_definitions: HashMap::new(),
            function_return_types: HashMap::new(),
            _current_receiver_type: None,
            _current_receiver_name: None,
            current_type_definition: None,
            result_inner_types: HashMap::new(),
            container_element_types: HashMap::new(),
        }
    }

    pub fn allocate_register(&mut self) -> u32 {
        let register = self.register_counter;
        self.register_counter += 1;
        register
    }

    pub fn allocate_label(&mut self, prefix: &str) -> Label {
        let label = Label::new(format!("{}_{}", prefix, self.label_counter));
        self.label_counter += 1;
        label
    }

    pub fn set_variable_type(&mut self, name: String, var_type: IRType) {
        self.variable_types.insert(name, var_type);
    }

    pub fn get_variable_type(&self, name: &str) -> Option<&IRType> {
        self.variable_types.get(name)
    }

    pub fn set_register_type(&mut self, register: u32, ty: IRType) {
        self.register_types.insert(register, ty);
    }

    pub fn get_register_type(&self, register: u32) -> Option<&IRType> {
        self.register_types.get(&register)
    }

    pub fn push_loop_labels(&mut self, break_label: String, continue_label: String) {
        self.break_stack.push(break_label);
        self.continue_stack.push(continue_label);
    }

    pub fn pop_loop_labels(&mut self) {
        self.break_stack.pop();
        self.continue_stack.pop();
    }

    pub fn current_break_label(&self) -> Option<&String> {
        self.break_stack.last()
    }

    pub fn current_continue_label(&self) -> Option<&String> {
        self.continue_stack.last()
    }

    pub fn create_label(&mut self, name: &str) -> Label {
        self.allocate_label(name)
    }

    /// Track ownership invalidation for move semantics
    pub fn invalidate_value(&mut self, value: IRValue) {
        if let IRValue::Variable(name) = value {
            let _ = self.variable_types.shift_remove(&name);
        }
    }

    /// Track borrow creation for lifetime validation
    pub fn track_borrow(&mut self, source: IRValue, reference: IRValue) {
        if let (IRValue::Variable(source_name), IRValue::Register(ref_id)) = (source, reference) {
            self.variable_types
                .entry(format!("borrow_{}_{}", source_name, ref_id))
                .or_insert(IRType::Pointer(Box::new(IRType::Void)));
        }
    }

    pub fn get_or_add_string(&mut self, s: &str) -> u32 {
        if let Some(&id) = self.string_table.get(s) {
            return id;
        }
        let id = self.string_table.len() as u32;
        self.string_table.insert(s.to_string(), id);
        id
    }

    /// Get the tag value for an enum variant based on definition order
    pub fn get_enum_variant_tag(
        &self,
        _enum_name: &str,
        variant_name: &str,
    ) -> Result<IRValue, String> {
        let tag = variant_name.bytes().enumerate().fold(0u32, |acc, (i, b)| {
            acc.wrapping_add((b as u32) * (i as u32 + 1))
        }) % 256;
        Ok(IRValue::Integer(tag as i64))
    }

    pub fn define_variable(&mut self, name: &str, value: IRValue) {
        // Check if we already have an explicit type for this variable (from type annotation)
        // Only infer type from value if we don't have one already
        let existing_type = self.variable_types.get(name).cloned();
        let has_explicit_type = existing_type.is_some();

        let var_type = if let Some(explicit) = existing_type.clone() {
            explicit
        } else {
            match &value {
                IRValue::Integer(_) => IRType::Integer,
                IRValue::Float(_) => IRType::Float,
                IRValue::Boolean(_) => IRType::Boolean,
                IRValue::StringConstant(_) | IRValue::String(_) => IRType::String,
                IRValue::ByteArray(bytes) => {
                    if bytes.is_empty() {
                        IRType::Array(Box::new(IRType::Void))
                    } else {
                        IRType::Array(Box::new(IRType::Integer))
                    }
                }
                IRValue::Register(reg) => {
                    let reg_type = self.register_types.get(reg).cloned();
                    if name == "entryOpt" {
                    }
                    reg_type.unwrap_or(IRType::Void)
                }
                IRValue::Struct { .. } | IRValue::Array(_) => value.get_type(),
                _ => IRType::Void,
            }
        };
        
        if name == "entryOpt" {
        }

        // Preserve explicit annotation but still record it in the map for downstream use
        if existing_type.is_none() {
            self.set_variable_type(name.to_string(), var_type.clone());
        }

        // Propagate inner types from Result<T,E>
        if let IRValue::Register(reg) = &value {
            if let Some(inner_type) = self.result_inner_types.get(&format!("reg_{}", reg)).cloned() {
                self.result_inner_types.insert(name.to_string(), inner_type);
            }
        }
        
        // Propagate container element types (Vec<T>, Option<T>, etc.)
        if let IRValue::Register(reg) = &value {
            if let Some(elem_type) = self.container_element_types.get(&format!("reg_{}", reg)).cloned() {
                self.container_element_types.insert(name.to_string(), elem_type);
            }
        }

        if var_type != IRType::Void && !self.local_variables.iter().any(|lv| lv.name == name) {
            let local = LocalVariable::new(name, var_type);
            self.local_variables.push(local);
        }
    }
}

impl Default for GenerationContext {
    fn default() -> Self {
        Self::new()
    }
}
