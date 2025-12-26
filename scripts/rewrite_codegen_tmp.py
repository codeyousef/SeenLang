from pathlib import Path
import re

backup = Path('temp_backup/seen_ir/src/generator.rs')
out_path = Path('seen_ir/src/generator/codegen.rs')
text = backup.read_text()

crate_use = (
    "use crate::{\n"
    "    function::IRFunction,\n"
    "    instruction::{BinaryOp, IRSelectArm, Instruction, Label, ScopeKind, UnaryOp},\n"
    "    module::IRModule,\n"
    "    value::{IRType, IRValue},\n"
    "    IRError, IRProgram, IRResult,\n"
    "};\n"
)
inject = crate_use + "use super::context::GenerationContext;\n"
if crate_use not in text:
    raise SystemExit('crate_use block not found')
text = text.replace(crate_use, inject, 1)

context_pattern = re.compile(
    r"\n/// Context for IR generation.*?impl Default for GenerationContext {\n    fn default\(\) -> Self {\n        Self::new\(\)\n    }\n}\n",
    re.S,
)
text, n = context_pattern.subn("\n", text, 1)
if n != 1:
    raise SystemExit(f'context block removal unexpected: {n}')

for name in [
    'convert_ast_type_to_ir',
    'register_struct_type',
    'register_enum_type',
    'register_class_type',
    'register_type_alias',
    'register_interface_type',
]:
    pattern = re.compile(rf"\n    fn {name}\(.*?\n    }}\n", re.S)
    text, removed = pattern.subn('\n', text, 1)
    if removed != 1:
        raise SystemExit(f'function {name} removal unexpected: {removed}')

text = text.replace('use std::collections::HashMap;\n', '')
out_path.write_text(text)
print('codegen.rs regenerated successfully')
