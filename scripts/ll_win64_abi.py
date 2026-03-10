#!/usr/bin/env python3
"""Transform LLVM IR for Windows x64 ABI compatibility.

On Windows x64, structs > 8 bytes are passed by pointer (callee gets ptr in rcx/rdx/r8/r9).
LLVM IR generated for Linux SysV ABI passes them by value (struct fields in registers).
This script rewrites the IR so struct parameters use `byval` attribute, and struct
return values use `sret`, matching the Windows calling convention.

Usage: python3 ll_win64_abi.py input.ll output.ll
"""
import re
import sys

def transform_ll_for_win64(content):
    # Collect all struct type definitions and their sizes
    struct_types = {}
    for m in re.finditer(r'^(%\w+)\s*=\s*type\s*\{([^}]*)\}', content, re.MULTILINE):
        name = m.group(1)
        fields = m.group(2).strip()
        # Rough size estimate: i64=8, ptr=8, i32=4, i16=2, i8=1, i1=1, nested structs vary
        size = 0
        for field in fields.split(','):
            field = field.strip()
            if not field:
                continue
            if field.startswith('%'):
                size += 16  # conservative estimate for nested structs
            elif 'i64' in field or 'ptr' in field or 'double' in field:
                size += 8
            elif 'i32' in field or 'float' in field:
                size += 4
            elif 'i16' in field:
                size += 2
            else:
                size += 1
        struct_types[name] = size

    # Struct types that need byval (> 8 bytes)
    byval_types = {name for name, size in struct_types.items() if size > 8}

    if not byval_types:
        return content

    # Update target triple and data layout
    content = re.sub(
        r'target triple = "[^"]*"',
        'target triple = "x86_64-w64-windows-gnu"',
        content
    )
    content = re.sub(
        r'target datalayout = "[^"]*"',
        'target datalayout = "e-m:w-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"',
        content
    )

    # Fix main: void -> i32, and only change "ret void" -> "ret i32 0" inside @main
    # On Windows, main() must return i32 (int), not void.
    lines = content.split('\n')
    new_lines = []
    in_main = False
    for line in lines:
        if re.match(r'define void @main\(i32 %\w+, ptr %\w+\)', line):
            line = re.sub(
                r'define void @main\(i32 (%\w+), ptr (%\w+)\)',
                r'define i32 @main(i32 \1, ptr \2)',
                line
            )
            in_main = True
        elif in_main and line.startswith('}'):
            in_main = False
        elif in_main and line.strip() == 'ret void':
            line = line.replace('ret void', 'ret i32 0')
        new_lines.append(line)
    content = '\n'.join(new_lines)

    # Pattern for struct types that need byval
    type_pattern = '|'.join(re.escape(t) for t in byval_types)

    # Transform declare/define statements: change struct params to ptr byval(Type) and struct returns to void + sret
    lines = content.split('\n')
    new_lines = []
    # Track which functions return structs (need sret transform at call sites)
    sret_functions = set()
    # Track sret info for defined functions (to fix ret instructions)
    # Maps function name -> (ret_type, sret_param_name)
    sret_defines = {}
    # Current function being sret-transformed (for fixing ret instructions)
    current_sret_type = None
    current_sret_param = None

    for line in lines:
        # Handle declare/define lines
        decl_match = re.match(r'^(declare|define)\s+(.*)', line)
        if decl_match:
            keyword = decl_match.group(1)
            rest = decl_match.group(2)

            # Check if return type is a byval struct
            ret_match = re.match(r'(%\w+)\s+(@[\w.]+)\(([^)]*)\)(.*)', rest)  # @[\w.] matches dots in names
            if ret_match and ret_match.group(1) in byval_types:
                ret_type = ret_match.group(1)
                func_name = ret_match.group(2)
                params = ret_match.group(3)
                attrs = ret_match.group(4)
                sret_functions.add(func_name)

                # For define: the sret param needs a name
                sret_param_name = '%_sret_out'
                if keyword == 'define':
                    current_sret_type = ret_type
                    current_sret_param = sret_param_name

                # Transform params
                new_params = f'ptr sret({ret_type}) {sret_param_name}' if keyword == 'define' else f'ptr sret({ret_type})'
                if params.strip():
                    for param in split_params(params):
                        new_params += ', ' + transform_param(param, byval_types)
                new_line = f'{keyword} void {func_name}({new_params}){attrs}'
                new_lines.append(new_line)
                continue

            else:
                # Not an sret function define - clear tracking
                current_sret_type = None
                current_sret_param = None

            # Check params for struct types
            param_match = re.match(r'(\S+)\s+(@[\w.]+)\(([^)]*)\)(.*)', rest)  # @[\w.] matches dots in names
            if param_match:
                ret_type = param_match.group(1)
                func_name = param_match.group(2)
                params = param_match.group(3)
                attrs = param_match.group(4)

                new_params = []
                for param in split_params(params):
                    new_params.append(transform_param(param, byval_types))

                new_line = f'{keyword} {ret_type} {func_name}({", ".join(new_params)}){attrs}'
                new_lines.append(new_line)
                continue

        # Fix ret instructions inside sret-transformed define blocks
        elif current_sret_type and current_sret_param:
            # Only match closing brace at column 0 (function end), not nested braces
            if re.match(r'^\}', line):
                current_sret_type = None
                current_sret_param = None
            else:
                # Check for "ret %StructType %val"
                ret_match = re.match(
                    rf'(\s+)ret {re.escape(current_sret_type)} (%\w+)',
                    line
                )
                if ret_match:
                    indent = ret_match.group(1)
                    val = ret_match.group(2)
                    new_lines.append(f'{indent}store {current_sret_type} {val}, ptr {current_sret_param}')
                    new_lines.append(f'{indent}ret void')
                    continue

        new_lines.append(line)

    content = '\n'.join(new_lines)

    # Now transform call sites
    # This is the hard part - we need to:
    # 1. For calls passing struct by value: alloca, store, pass ptr byval
    # 2. For calls returning struct: alloca sret, pass as first arg

    # Transform call sites with struct args
    # Pattern: call void @func(%SeenString %var)
    # -> %tmp = alloca %SeenString; store %SeenString %var, ptr %tmp; call void @func(ptr byval(%SeenString) %tmp)

    lines = content.split('\n')
    new_lines = []
    alloca_counter = [0]

    for line in lines:
        transformed = transform_call_line(line, byval_types, sret_functions, alloca_counter)
        if isinstance(transformed, list):
            new_lines.extend(transformed)
        else:
            new_lines.append(transformed)

    return '\n'.join(new_lines)


def split_params(params_str):
    """Split parameter list respecting parentheses nesting."""
    params = []
    depth = 0
    current = ''
    for ch in params_str:
        if ch == '(' :
            depth += 1
            current += ch
        elif ch == ')':
            depth -= 1
            current += ch
        elif ch == ',' and depth == 0:
            params.append(current.strip())
            current = ''
        else:
            current += ch
    if current.strip():
        params.append(current.strip())
    return params


def transform_param(param, byval_types):
    """Transform a single parameter if it's a byval struct type."""
    # Match patterns like: %SeenString %name, %SeenString (no name in declare), etc.
    for t in byval_types:
        escaped = re.escape(t)
        # In declare: just type name, no variable
        if re.match(rf'^{escaped}$', param.strip()):
            return f'ptr byval({t})'
        # In define/call: type + variable
        m = re.match(rf'^{escaped}\s+(%\w+)$', param.strip())
        if m:
            return f'ptr byval({t}) {m.group(1)}'
        # With attributes like nonnull
        m = re.match(rf'^{escaped}\s+(nonnull\s+)?(%\w+)$', param.strip())
        if m:
            attrs = m.group(1) or ''
            return f'ptr byval({t}) {attrs}{m.group(2)}'
    return param


def transform_call_line(line, byval_types, sret_functions, counter):
    """Transform a call instruction to use byval/sret where needed."""
    stripped = line.strip()

    # Skip non-call lines
    if 'call ' not in stripped:
        return line

    # Check if this call has any struct args or struct return
    has_struct_arg = any(t in stripped for t in byval_types)

    if not has_struct_arg:
        return line

    indent = line[:len(line) - len(line.lstrip())]

    # Parse the call
    # Pattern: %result = call %RetType @func(%Type1 %a, %Type2 %b)
    # or: call void @func(%Type1 %a)

    # Check for struct return
    ret_match = re.match(r'\s*(%\w+)\s*=\s*call\s+(%\w+)\s+(@[\w.]+)\(([^)]*)\)', line)
    if ret_match and ret_match.group(2) in byval_types and ret_match.group(3) in sret_functions:
        result_var = ret_match.group(1)
        ret_type = ret_match.group(2)
        func_name = ret_match.group(3)
        args_str = ret_match.group(4)

        lines_out = []
        sret_var = f'%_sret_{counter[0]}'
        counter[0] += 1
        lines_out.append(f'{indent}{sret_var} = alloca {ret_type}')

        # Transform args
        new_args, extra_lines = transform_call_args(args_str, byval_types, indent, counter)
        lines_out.extend(extra_lines)

        lines_out.append(f'{indent}call void {func_name}(ptr sret({ret_type}) {sret_var}, {new_args})')
        lines_out.append(f'{indent}{result_var} = load {ret_type}, ptr {sret_var}')
        return lines_out

    # Check for struct args (no struct return)
    call_match = re.match(r'(\s*(?:%\w+\s*=\s*)?call\s+\S+\s+@[\w.]+)\(([^)]*)\)', line)
    if call_match:
        prefix = call_match.group(1)
        args_str = call_match.group(2)

        new_args, extra_lines = transform_call_args(args_str, byval_types, indent, counter)
        if extra_lines:
            result = extra_lines + [f'{prefix}({new_args})']
            return result

    return line


def transform_call_args(args_str, byval_types, indent, counter):
    """Transform call arguments, adding alloca+store for struct values."""
    args = split_params(args_str)
    new_args = []
    extra_lines = []

    for arg in args:
        transformed = False
        for t in byval_types:
            # Match: %Type %var  or  %Type %var (with insertvalue result)
            m = re.match(rf'^{re.escape(t)}\s+(%\w+)$', arg.strip())
            if m:
                var = m.group(1)
                tmp_var = f'%_byval_{counter[0]}'
                counter[0] += 1
                extra_lines.append(f'{indent}{tmp_var} = alloca {t}')
                extra_lines.append(f'{indent}store {t} {var}, ptr {tmp_var}')
                new_args.append(f'ptr byval({t}) {tmp_var}')
                transformed = True
                break
        if not transformed:
            new_args.append(arg)

    return ', '.join(new_args), extra_lines


if __name__ == '__main__':
    if len(sys.argv) != 3:
        print(f'Usage: {sys.argv[0]} input.ll output.ll', file=sys.stderr)
        sys.exit(1)

    with open(sys.argv[1]) as f:
        content = f.read()

    result = transform_ll_for_win64(content)

    with open(sys.argv[2], 'w') as f:
        f.write(result)

    print(f'Transformed {sys.argv[1]} -> {sys.argv[2]}', file=sys.stderr)
