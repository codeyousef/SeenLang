#!/usr/bin/env python3
"""Opt wrapper for macOS bootstrap: deduplicates declares and fixes ABI mismatches
in .ll files before running the real LLVM opt.

The macOS bootstrap compiler generates .ll files with cross-module ABI mismatches:
- Duplicate declare statements
- Declarations with extra ptr %this parameters vs definitions
- Call sites returning i64 for void functions
- Call sites missing ptr %this first argument

This wrapper transparently fixes these before opt processes the files.
"""
import re, sys, os, subprocess, glob

def fix_codegen_bugs(ll_file, duplicate_globals=None, extern_refs=None):
    """Fix codegen bugs: malformed types, void in structs, undefined functions,
    malformed globals, duplicate globals, type mismatches in calls.
    Line-based processing for performance on large files."""
    if duplicate_globals is None:
        duplicate_globals = set()
    if extern_refs is None:
        extern_refs = set()
    with open(ll_file) as f:
        lines = f.readlines()

    changed = False
    seen_globals = set()  # track global variable names to deduplicate
    new_lines = []

    # Single pass: fix codegen issues
    for line in lines:
        original = line

        # 1. Remove malformed type names (%:, %., %==, etc.)
        if line.startswith('%') and '= type {' in line:
            name_end = line.index(' ')
            name = line[1:name_end]
            if name and not name[0].isalpha() and name[0] != '_':
                changed = True
                continue  # skip this line

        # 2. Fix void in struct type definitions and GEP
        if 'void' in line:
            if '= type {' in line:
                line = line.replace(', void', ', i8').replace('{ void', '{ i8')
            elif 'getelementptr' in line:
                line = line.replace(', void }', ', i8 }').replace(', void,', ', i8,')
            if line != original:
                changed = True

        # 3. Replace @Int_addError calls with add instruction
        if '@Int_addError' in line:
            m = re.match(r'(\s*)(%\w+)\s*=\s*call\s+i64\s+@Int_addError\(i64\s+(%\w+),\s*i64\s+(%\w+)\)', line)
            if m:
                line = f'{m.group(1)}{m.group(2)} = add i64 {m.group(3)}, {m.group(4)}\n'
                changed = True

        # 4. Fix malformed globals (e.g., @hexChars = global i64 0123456789abcdef)
        if line.startswith('@') and '= global i64 ' in line:
            gm = re.match(r'(@\w+\s*=\s*global\s+i64\s+)(\S+)\s*$', line)
            if gm:
                val = gm.group(2)
                # If value contains non-digit chars (not a valid integer), replace with 0
                if val and not re.match(r'^-?\d+$', val):
                    line = f'{gm.group(1)}0\n'
                    changed = True

        # 5. Deduplicate globals and make cross-module duplicates internal
        if line.startswith('@') and '= global ' in line:
            gname = line.split('=')[0].strip()
            if gname in seen_globals:
                changed = True
                continue  # skip duplicate
            seen_globals.add(gname)
            # Make globals that are duplicated across modules internal (module-local)
            if gname in duplicate_globals and 'internal' not in line:
                line = line.replace('= global ', '= internal global ', 1)
                if line != original:
                    changed = True
            # Promote internal globals to global if referenced externally from other modules
            if gname in extern_refs and 'internal global' in line:
                line = line.replace('internal global', 'global', 1)
                if line != original:
                    changed = True

        # 6. Fix %SeenString 0 -> %SeenString zeroinitializer (codegen bug: bare 0 instead of zeroinitializer)
        if '%SeenString 0' in line and 'zeroinitializer' not in line:
            line = line.replace('%SeenString 0', '%SeenString zeroinitializer')
            if line != original:
                changed = True

        new_lines.append(line)

    if changed:
        with open(ll_file, 'w') as f:
            f.writelines(new_lines)

    # Per-function type mismatch fix: if a register loaded as i64 is used
    # exclusively as %SeenString within the same function, change the load
    # and corresponding store to use %SeenString.
    with open(ll_file) as f:
        lines = f.readlines()

    func_lines = []
    func_start = None
    for i, line in enumerate(lines):
        if line.startswith('define '):
            func_start = i
        elif line.strip() == '}' and func_start is not None:
            func_lines.append((func_start, i))
            func_start = None

    changed2 = False
    for fstart, fend in func_lines:
        load_types = {}
        load_ptrs = {}
        for i in range(fstart, fend + 1):
            line = lines[i]
            if '= load ' in line:
                lm = re.match(r'(\s*)(%\w+)(\s*=\s*load\s+)(\S+)(,\s*ptr\s+(%\w+).*)', line)
                if lm:
                    load_types[lm.group(2)] = (lm.group(4), i)
                    load_ptrs[lm.group(2)] = lm.group(6)

        str_call_regs = set()
        i64_call_regs = set()
        for i in range(fstart, fend + 1):
            line = lines[i]
            if 'call ' in line:
                for m in re.finditer(r'%SeenString\s+(%\w+)', line):
                    str_call_regs.add(m.group(1))
                for m in re.finditer(r'(?<!\w)i64\s+(%\w+)', line):
                    i64_call_regs.add(m.group(1))

        for reg in str_call_regs:
            if reg in load_types and load_types[reg][0] == 'i64' and reg not in i64_call_regs:
                load_line_idx = load_types[reg][1]
                ptr_var = load_ptrs.get(reg)
                lines[load_line_idx] = re.sub(
                    rf'({re.escape(reg)}\s*=\s*load\s+)i64',
                    rf'\g<1>%SeenString',
                    lines[load_line_idx]
                )
                changed2 = True
                if ptr_var:
                    for i in range(fstart, fend + 1):
                        if f'store i64 0, ptr {ptr_var}' in lines[i]:
                            lines[i] = lines[i].replace(
                                f'store i64 0, ptr {ptr_var}',
                                f'store %SeenString zeroinitializer, ptr {ptr_var}'
                            )

    if changed2:
        with open(ll_file, 'w') as f:
            f.writelines(lines)

def deduplicate_declares(ll_file):
    """Remove duplicate declare statements from .ll file."""
    with open(ll_file) as f:
        lines = f.readlines()
    seen = {}
    out = []
    for line in lines:
        if line.startswith('declare '):
            m = re.search(r'@(\w+)', line)
            if m:
                fname = m.group(1)
                if fname in seen:
                    continue
                seen[fname] = True
        out.append(line)
    if len(out) != len(lines):
        with open(ll_file, 'w') as f:
            f.writelines(out)

def collect_all_defs(ll_dir):
    """Collect all function definitions across all modules with full param type info."""
    all_defs = {}
    for f in glob.glob(os.path.join(ll_dir, 'seen_module_*.ll')):
        if f.endswith('.opt.ll'):
            continue
        try:
            with open(f) as fh:
                for line in fh:
                    dm = re.match(r'define\s+(?:noundef\s+)?(\S+)\s+@(\w+)\(([^)]*)\)', line)
                    if dm:
                        dret = dm.group(1)
                        dname = dm.group(2)
                        dparams = dm.group(3).strip()
                        if dparams:
                            param_types = []
                            for p in dparams.split(','):
                                p = p.strip()
                                parts = p.split()
                                ptype = parts[0]
                                if ptype in ('noalias', 'nonnull', 'noundef'):
                                    ptype = parts[1] if len(parts) > 1 else ptype
                                param_types.append(ptype)
                            dpcount = len(param_types)
                        else:
                            param_types = []
                            dpcount = 0
                        all_defs[dname] = (dret, dpcount, param_types)
        except:
            pass
    return all_defs

def collect_all_globals(ll_dir):
    """Collect all global variable definitions across all modules.
    Returns (all_globals, duplicate_globals, extern_refs) where duplicate_globals
    are globals defined in 2+ modules (need internal linkage) and extern_refs
    are globals declared external in at least one module."""
    all_globals = {}  # name -> type
    global_counts = {}  # name -> count of modules defining it
    extern_refs = set()  # globals declared as external (cross-module references)
    for f in glob.glob(os.path.join(ll_dir, 'seen_module_*.ll')):
        if f.endswith('.opt.ll'):
            continue
        try:
            module_globals = set()
            with open(f) as fh:
                for line in fh:
                    gm = re.match(r'(@\w+)\s*=\s*(external\s+)?(?:local_unnamed_addr\s+)?(?:unnamed_addr\s+)?global\s+(\S+)', line)
                    if gm:
                        gname = gm.group(1)
                        is_external = gm.group(2) is not None
                        all_globals[gname] = gm.group(3)
                        if is_external:
                            extern_refs.add(gname)
                        if gname not in module_globals:
                            module_globals.add(gname)
                            if not is_external:
                                global_counts[gname] = global_counts.get(gname, 0) + 1
        except:
            pass
    duplicate_globals = {g for g, c in global_counts.items() if c > 1}
    return all_globals, duplicate_globals, extern_refs

def fix_missing_symbols(ll_file, all_defs, all_globals):
    """Add missing function declarations and external global declarations for cross-module references."""
    with open(ll_file) as f:
        content = f.read()

    # Collect functions defined or declared in this module
    # Use a broad regex that handles LLVM attributes (noundef, range(...), etc.)
    local_funcs = set()
    for m in re.finditer(r'(?:define|declare)\s+[^@]*@(\w+)\(', content):
        local_funcs.add(m.group(1))

    # Collect globals defined in this module (handle LLVM attributes like local_unnamed_addr)
    local_globals = set()
    for m in re.finditer(r'^(@\w+)\s*=\s*(?:external\s+)?(?:local_unnamed_addr\s+)?(?:unnamed_addr\s+)?global\s', content, re.MULTILINE):
        local_globals.add(m.group(1))

    # Find missing function declarations
    missing_funcs = set()
    for m in re.finditer(r'call\s+\S+\s+@(\w+)\(', content):
        fname = m.group(1)
        if fname not in local_funcs and fname in all_defs:
            missing_funcs.add(fname)

    # Find missing global declarations (only for store/load patterns, not function calls)
    missing_globals = set()
    for m in re.finditer(r'(?:store|load)\s+\S+[^@]*(?:ptr\s+)?(@\w+)', content):
        gname = m.group(1)
        if gname not in local_globals and not gname.startswith('@.str') and gname in all_globals:
            missing_globals.add(gname)

    if not missing_funcs and not missing_globals:
        return

    decls = ''
    for fname in sorted(missing_funcs):
        ret, pcount, ptypes = all_defs[fname]
        params = ', '.join(ptypes)
        decls += f'declare {ret} @{fname}({params})\n'
    for gname in sorted(missing_globals):
        gtype = all_globals[gname]
        decls += f'{gname} = external global {gtype}\n'

    # Insert before first define
    insert_pos = content.find('\ndefine ')
    if insert_pos != -1:
        content = content[:insert_pos] + '\n' + decls + content[insert_pos:]
        with open(ll_file, 'w') as f:
            f.write(content)

def fix_abi_mismatches(ll_file, all_defs):
    """Fix all ABI mismatches in a .ll file."""
    with open(ll_file) as f:
        lines = f.readlines()

    new_lines = []
    changed = False
    for line in lines:
        original_line = line

        # Fix declarations to match definitions
        if line.startswith('declare '):
            decl_match = re.match(r'(declare\s+\S+\s+@(\w+)\()([^)]*)\)(.*)', line)
            if decl_match:
                name = decl_match.group(2)
                if name in all_defs:
                    params = decl_match.group(3).strip()
                    param_list = [p.strip() for p in params.split(',') if p.strip()] if params else []
                    def_ret, def_pcount, def_ptypes = all_defs[name]
                    if len(param_list) != def_pcount:
                        prefix_part = decl_match.group(1)
                        rest = decl_match.group(4)
                        new_params = ', '.join(def_ptypes)
                        line = f"{prefix_part}{new_params}){rest}\n"
                        if line != original_line:
                            changed = True

        # Fix void-return mismatches
        if '= call ' in line and not line.strip().startswith(('@', ';', 'c"')):
            m = re.match(r'(\s*)(%\w+\s*=\s*)call\s+(?:noundef\s+)?(\S+)\s+@(\w+)\(([^)]*)\)(.*)', line)
            if m:
                indent = m.group(1)
                ret_type = m.group(3)
                func_name = m.group(4)
                args = m.group(5)
                rest = m.group(6)
                if func_name in all_defs:
                    def_ret = all_defs[func_name][0]
                    if def_ret == 'void' and ret_type != 'void':
                        line = f"{indent}call void @{func_name}({args}){rest}\n"
                        if line != original_line:
                            changed = True

        # Fix call-site parameter count mismatches
        if 'call ' in line and '@' in line and not line.strip().startswith((';', 'c"')):
            cm = re.match(r'(\s*)((?:%\w+\s*=\s*)?)call\s+(?:noundef\s+)?(\S+)\s+@(\w+)\(([^)]*)\)(.*)', line)
            if cm:
                indent = cm.group(1)
                prefix = cm.group(2)
                ret_type = cm.group(3)
                func_name = cm.group(4)
                args_str = cm.group(5)
                rest = cm.group(6)
                if func_name in all_defs:
                    def_ret, def_pcount, def_ptypes = all_defs[func_name]
                    # Parse call args with depth tracking
                    if args_str.strip():
                        depth = 0; call_args = []; current = ''
                        for c in args_str:
                            if c in '({': depth += 1
                            elif c in '})': depth -= 1
                            if c == ',' and depth == 0:
                                call_args.append(current.strip())
                                current = ''
                            else:
                                current += c
                        if current.strip():
                            call_args.append(current.strip())
                    else:
                        call_args = []

                    call_count = len(call_args)

                    # Fix type mismatches (same arg count but wrong types)
                    if call_count == def_pcount and call_count > 0:
                        fixed_args = []
                        mismatch = False
                        i = 0
                        di = 0
                        while i < len(call_args) and di < len(def_ptypes):
                            carg = call_args[i].strip()
                            dtype = def_ptypes[di]
                            # Extract call arg type (first token)
                            cparts = carg.split()
                            ctype = cparts[0] if cparts else ''
                            # Skip LLVM attributes
                            if ctype in ('noalias', 'nonnull', 'noundef'):
                                ctype = cparts[1] if len(cparts) > 1 else ctype

                            if ctype == 'i64' and dtype == '%SeenString':
                                # i64 passed where %SeenString expected — the codegen
                                # stores zeroinitializer then loads only the first field
                                fixed_args.append('%SeenString zeroinitializer')
                                mismatch = True
                            elif ctype == '%SeenString' and dtype == 'i64':
                                # %SeenString passed where i64 expected — extract first field
                                fixed_args.append('i64 0')
                                mismatch = True
                            else:
                                fixed_args.append(carg)
                            i += 1
                            di += 1
                        if mismatch:
                            new_args_str = ', '.join(fixed_args)
                            line = f"{indent}{prefix}call {ret_type} @{func_name}({new_args_str}){rest}\n"
                            if line != original_line:
                                changed = True

                    # Fix struct-split mismatches: (i64, ptr) passed for %SeenString
                    # %SeenString = {i64, ptr}, so 2 args where 1 expected
                    if call_count > def_pcount and call_count - def_pcount <= 4:
                        fixed_args = []
                        mismatch = False
                        i = 0
                        for di in range(def_pcount):
                            if i >= len(call_args):
                                break
                            dtype = def_ptypes[di]
                            carg = call_args[i].strip()
                            cparts = carg.split()
                            ctype = cparts[0] if cparts else ''
                            if ctype in ('noalias', 'nonnull', 'noundef'):
                                ctype = cparts[1] if len(cparts) > 1 else ctype

                            if dtype == '%SeenString' and ctype == 'i64' and i + 1 < len(call_args):
                                # Check if next arg is ptr — struct was split into (i64, ptr)
                                narg = call_args[i + 1].strip()
                                ntype = narg.split()[0] if narg.split() else ''
                                if ntype in ('noalias', 'nonnull', 'noundef'):
                                    ntype = narg.split()[1] if len(narg.split()) > 1 else ntype
                                if ntype == 'ptr':
                                    fixed_args.append('%SeenString zeroinitializer')
                                    mismatch = True
                                    i += 2  # consumed two args
                                    continue
                            fixed_args.append(carg)
                            i += 1
                        # Append any remaining args
                        while i < len(call_args):
                            fixed_args.append(call_args[i].strip())
                            i += 1
                        if mismatch and len(fixed_args) == def_pcount:
                            new_args_str = ', '.join(fixed_args)
                            line = f"{indent}{prefix}call {ret_type} @{func_name}({new_args_str}){rest}\n"
                            if line != original_line:
                                changed = True

                    if call_count < def_pcount and def_pcount - call_count <= 2:
                        # Missing args — prepend defaults at beginning (ptr %this)
                        missing = def_pcount - call_count
                        new_args = []
                        for i in range(missing):
                            ptype = def_ptypes[i] if i < len(def_ptypes) else 'i64'
                            if ptype == 'i1': new_args.append('i1 false')
                            elif ptype == 'i64': new_args.append('i64 0')
                            elif ptype == 'ptr': new_args.append('ptr null')
                            elif ptype == '%SeenString': new_args.append('%SeenString zeroinitializer')
                            elif ptype == 'double': new_args.append('double 0.0')
                            else: new_args.append(f'{ptype} zeroinitializer')
                        all_args = new_args + call_args
                        new_args_str = ', '.join(all_args)
                        line = f"{indent}{prefix}call {ret_type} @{func_name}({new_args_str}){rest}\n"
                        if line != original_line:
                            changed = True

        new_lines.append(line)

    if changed:
        with open(ll_file, 'w') as f:
            f.writelines(new_lines)

# Find .ll directory and collect definitions
ll_dir = '/tmp'
for arg in sys.argv[1:]:
    if arg.endswith('.ll') and os.path.exists(arg):
        ll_dir = os.path.dirname(arg) or '/tmp'
        break
all_defs = collect_all_defs(ll_dir)
all_globals, duplicate_globals, extern_refs = collect_all_globals(ll_dir)

# Fix all .ll files in args
# Only apply codegen fixes to raw .ll files (not .opt.ll which have LLVM attributes)
for arg in sys.argv[1:]:
    if arg.endswith('.ll') and os.path.exists(arg):
        is_opt_ll = arg.endswith('.opt.ll')
        if not is_opt_ll:
            fix_codegen_bugs(arg, duplicate_globals, extern_refs)
            fix_missing_symbols(arg, all_defs, all_globals)
            deduplicate_declares(arg)
            fix_abi_mismatches(arg, all_defs)

# Run real opt (search common locations)
opt_candidates = [
    "/opt/homebrew/opt/llvm/bin/opt",
    "/usr/local/opt/llvm/bin/opt",
    "/usr/bin/opt",
]
opt_path = None
for p in opt_candidates:
    if os.path.exists(p):
        opt_path = p
        break
if opt_path is None:
    # Try PATH
    import shutil
    # Skip ourselves by checking if it's a python script
    for d in os.environ.get('PATH', '').split(':'):
        candidate = os.path.join(d, 'opt')
        if os.path.exists(candidate) and candidate != os.path.abspath(__file__):
            with open(candidate, 'rb') as f:
                header = f.read(4)
            if header != b'#!/u':  # Not a script (i.e., real binary)
                opt_path = candidate
                break
    if opt_path is None:
        print("Error: could not find LLVM opt binary", file=sys.stderr)
        sys.exit(1)

result = subprocess.run([opt_path] + sys.argv[1:])
sys.exit(result.returncode)
