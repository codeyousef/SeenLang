#!/bin/bash
# Script to apply LLVM integration fixes to seen_ir

echo "=== Applying LLVM Integration Fixes ==="
echo

# Color codes
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

# Backup files first
echo -e "${YELLOW}Creating backups...${NC}"
cp seen_ir/src/codegen.rs seen_ir/src/codegen.rs.backup
cp seen_ir/src/types.rs seen_ir/src/types.rs.backup
echo -e "${GREEN}✓ Backups created${NC}"
echo

# Fix 1: Add Struct Declaration handler in codegen.rs
echo -e "${YELLOW}Fixing Declaration::Struct pattern...${NC}"
sed -i '/Declaration::Variable(var_decl) => {/,/}/a\
            Declaration::Struct(_struct_decl) => {\
                // TODO: Implement struct declaration code generation\
                return Err(CodeGenError::UnsupportedFeature(\
                    "Struct declarations not yet implemented in IR generation".to_string()\
                ));\
            }' seen_ir/src/codegen.rs

# Fix 2: Add For Statement handler in codegen.rs
echo -e "${YELLOW}Fixing Statement::For pattern...${NC}"
# This is trickier because we need to add it before DeclarationStatement
# First, let's use a more specific pattern
sed -i '/Statement::DeclarationStatement(decl) => self::generate_declaration(decl),/i\
            Statement::For(_for_stmt) => {\
                // TODO: Implement for loop code generation\
                return Err(CodeGenError::UnsupportedFeature(\
                    "For loops not yet implemented in IR generation".to_string()\
                ));\
            }' seen_ir/src/codegen.rs

# Fix 3: Add missing Expression handlers in codegen.rs
echo -e "${YELLOW}Fixing Expression patterns...${NC}"
# Find the end of the match expression and add before the closing brace
sed -i '/Expression::Assign(assign) => self::generate_assignment(assign),/a\
            Expression::StructLiteral(_struct_lit) => {\
                Err(CodeGenError::UnsupportedFeature(\
                    "Struct literals not yet implemented in IR generation".to_string()\
                ))\
            }\
            Expression::FieldAccess(_field_access) => {\
                Err(CodeGenError::UnsupportedFeature(\
                    "Field access not yet implemented in IR generation".to_string()\
                ))\
            }\
            Expression::ArrayLiteral(_array_lit) => {\
                Err(CodeGenError::UnsupportedFeature(\
                    "Array literals not yet implemented in IR generation".to_string()\
                ))\
            }\
            Expression::Index(_index_expr) => {\
                Err(CodeGenError::UnsupportedFeature(\
                    "Array indexing not yet implemented in IR generation".to_string()\
                ))\
            }\
            Expression::Range(_range_expr) => {\
                Err(CodeGenError::UnsupportedFeature(\
                    "Range expressions not yet implemented in IR generation".to_string()\
                ))\
            }' seen_ir/src/codegen.rs

# Fix 4: Add Struct Type handler in types.rs
echo -e "${YELLOW}Fixing Type::Struct pattern in types.rs...${NC}"
sed -i '/Type::Array(elem_type) => {/,/}/a\
            Type::Struct(_struct_name) => {\
                return Err(CodeGenError::UnsupportedFeature(\
                    "Struct types not yet implemented in IR generation".to_string()\
                ))\
            }' seen_ir/src/types.rs

echo
echo -e "${GREEN}✓ All fixes applied!${NC}"
echo
echo "Now testing compilation..."
LLVM_SYS_181_PREFIX=/usr/lib/llvm-18 cargo check --package seen_ir
if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓ seen_ir compiles successfully!${NC}"
else
    echo -e "${YELLOW}⚠ Compilation still has errors. You may need to apply fixes manually.${NC}"
    echo "Check the backup files if needed:"
    echo "  - seen_ir/src/codegen.rs.backup"
    echo "  - seen_ir/src/types.rs.backup"
fi