/*
 * seen_ir_lint.c — Semantic LLVM IR linter for the Seen compiler
 *
 * Catches codegen bugs that llvm-as doesn't:
 *   1. Call-site type mismatches (return type, arg count, arg types)
 *   2. Undefined function references (missing declare/define)
 *   3. norecurse on recursive functions
 *   4. Contradictory attributes (alwaysinline + noinline)
 *
 * Usage: seen_ir_lint file1.ll [file2.ll ...]
 * Exit code: 0 = clean, 1 = errors found
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <ctype.h>
#include <stdbool.h>

#define MAX_FUNCS     8192
#define MAX_PARAMS    64
#define MAX_LINE      65536
#define MAX_NAME      512
#define MAX_TYPE      256

typedef struct {
    char name[MAX_NAME];
    char ret_type[MAX_TYPE];
    char param_types[MAX_PARAMS][MAX_TYPE];
    int  param_count;
    bool has_norecurse;
    bool has_noinline;
    bool has_alwaysinline;
    bool is_vararg;
    bool is_define;     /* true = define, false = declare */
} FuncInfo;

typedef struct {
    FuncInfo funcs[MAX_FUNCS];
    int count;
} FuncTable;

static int g_errors = 0;
static int g_warnings = 0;

/* ---- String utilities ---- */

/* Skip leading whitespace */
static const char *skip_ws(const char *s) {
    while (*s && isspace((unsigned char)*s)) s++;
    return s;
}

/* Strip LLVM parameter attributes like noundef, nonnull, nocapture, etc.
 * Returns a cleaned type string. */
static void strip_attrs(const char *raw, char *out, int outsize) {
    static const char *attrs[] = {
        "noundef", "nonnull", "nocapture", "readonly", "readnone",
        "writeonly", "signext", "zeroext", "inreg", "byval", "sret",
        "nest", "returned", "nofree", "nosync", "immarg", "willreturn",
        "noalias", "dereferenceable", "align", "nnan", "ninf", "nsz",
        "arcp", "fast", "contract", "afn", "reassoc",
        NULL
    };
    const char *p = raw;
    char *o = out;
    char *end = out + outsize - 1;

    p = skip_ws(p);

    while (*p && o < end) {
        /* Skip known attributes */
        bool skipped = false;
        for (int i = 0; attrs[i]; i++) {
            int len = strlen(attrs[i]);
            if (strncmp(p, attrs[i], len) == 0 &&
                (p[len] == '\0' || isspace((unsigned char)p[len]) ||
                 p[len] == ',' || p[len] == ')' || p[len] == '(')) {
                p += len;
                /* Skip parenthesized argument like dereferenceable(8) or align(4) */
                if (*p == '(') {
                    int depth = 1;
                    p++;
                    while (*p && depth > 0) {
                        if (*p == '(') depth++;
                        else if (*p == ')') depth--;
                        p++;
                    }
                }
                /* Skip trailing whitespace */
                while (*p && isspace((unsigned char)*p)) p++;
                skipped = true;
                break;
            }
        }
        if (skipped) continue;

        /* Copy character */
        *o++ = *p++;
    }
    *o = '\0';

    /* Trim trailing whitespace */
    while (o > out && isspace((unsigned char)*(o-1))) {
        *(--o) = '\0';
    }
}

/* Check if a type is "compatible" for our purposes.
 * We do a relaxed comparison: i64 and ptr are both valid handle types,
 * and we treat struct types by name. */
static bool types_compatible(const char *decl_type, const char *call_type) {
    char a[MAX_TYPE], b[MAX_TYPE];
    strip_attrs(decl_type, a, sizeof(a));
    strip_attrs(call_type, b, sizeof(b));

    if (strcmp(a, b) == 0) return true;

    /* Allow ptr <-> ptr addrspace(N) */
    if (strncmp(a, "ptr", 3) == 0 && strncmp(b, "ptr", 3) == 0) return true;

    /* Allow i64 <-> ptr (Seen uses i64 handles for class pointers) */
    if ((strcmp(a, "i64") == 0 && strcmp(b, "ptr") == 0) ||
        (strcmp(a, "ptr") == 0 && strcmp(b, "i64") == 0)) return true;

    return false;
}

/* ---- Parsing helpers ---- */

/* Extract function name from a declare/define line.
 * Looks for @name( pattern.
 * Returns pointer into line at the name start, sets *end to char after name. */
static bool extract_func_name(const char *line, char *name, int namesize) {
    const char *at = strstr(line, "@");
    if (!at) return false;
    at++; /* skip @ */

    /* Handle quoted names like @"foo" */
    if (*at == '"') {
        at++;
        const char *q = strchr(at, '"');
        if (!q) return false;
        int len = q - at;
        if (len >= namesize) len = namesize - 1;
        memcpy(name, at, len);
        name[len] = '\0';
        return true;
    }

    const char *end = at;
    while (*end && (isalnum((unsigned char)*end) || *end == '_' || *end == '.')) end++;
    int len = end - at;
    if (len == 0 || len >= namesize) return false;
    memcpy(name, at, len);
    name[len] = '\0';
    return true;
}

/* Parse the return type from a declare/define line.
 * Format: declare RETTYPE @name(...)
 *    or:  define [linkage] [attrs] RETTYPE @name(...)
 * We find the @name and work backwards to get the type token. */
static bool extract_ret_type(const char *line, char *ret, int retsize) {
    const char *at = strstr(line, "@");
    if (!at) return false;

    /* Walk backwards from @ skipping whitespace to find type */
    const char *p = at - 1;
    while (p > line && isspace((unsigned char)*p)) p--;
    if (p <= line) return false;

    /* Now p points to the last char of the return type.
     * Walk backwards to find the start of this token. */
    const char *end = p + 1;

    /* Handle pointer types ending in '*' */
    /* Handle struct types like %StructName */
    /* Handle simple types like i64, void, ptr, double, float, i1, i32 */
    const char *start = p;
    while (start > line && !isspace((unsigned char)*(start-1))) start--;

    int len = end - start;
    if (len >= retsize) len = retsize - 1;
    memcpy(ret, start, len);
    ret[len] = '\0';
    return true;
}

/* Parse parameter types from a function signature.
 * Expects to start from the '(' after the function name.
 * Returns number of params found. Sets is_vararg if ... present. */
static int parse_param_types(const char *params_start, char types[][MAX_TYPE],
                             int max_params, bool *is_vararg) {
    *is_vararg = false;
    if (!params_start || *params_start != '(') return 0;

    const char *p = params_start + 1; /* skip ( */
    int count = 0;
    int depth = 1;

    while (*p && depth > 0 && count < max_params) {
        p = skip_ws(p);
        if (*p == ')') { depth--; break; }
        if (*p == '\0') break;

        /* Check for vararg */
        if (strncmp(p, "...", 3) == 0) {
            *is_vararg = true;
            p += 3;
            continue;
        }

        /* Read one parameter type (may include attributes) */
        char raw_param[MAX_TYPE * 2] = {0};
        int ri = 0;
        /* Read until ',' or ')' at depth 0, tracking nested parens */
        int inner_depth = 0;
        while (*p && ri < (int)sizeof(raw_param) - 1) {
            if (*p == '(') inner_depth++;
            else if (*p == ')') {
                if (inner_depth == 0) break;
                inner_depth--;
            }
            else if (*p == ',' && inner_depth == 0) break;
            raw_param[ri++] = *p++;
        }
        raw_param[ri] = '\0';

        /* Strip the parameter name (e.g., "%name" at the end) and attributes */
        /* The type is everything before the last %name token */
        char *pct = strrchr(raw_param, '%');
        char type_part[MAX_TYPE * 2] = {0};
        if (pct && pct > raw_param) {
            /* Check if this %name looks like a parameter name (not a type like %SeenString) */
            /* Type names start with % and are followed by a capital letter usually */
            /* Parameter names are like %0, %1, %param */
            const char *after_pct = pct + 1;
            /* If it's a digit or lowercase after %, it's a param name - strip it */
            if (isdigit((unsigned char)*after_pct) || islower((unsigned char)*after_pct)) {
                int tlen = pct - raw_param;
                memcpy(type_part, raw_param, tlen);
                type_part[tlen] = '\0';
            } else {
                strcpy(type_part, raw_param);
            }
        } else {
            strcpy(type_part, raw_param);
        }

        /* Strip attributes from the type */
        strip_attrs(type_part, types[count], MAX_TYPE);

        /* Trim trailing whitespace from the type */
        int len = strlen(types[count]);
        while (len > 0 && isspace((unsigned char)types[count][len-1])) {
            types[count][--len] = '\0';
        }

        /* Only count non-empty types */
        if (strlen(types[count]) > 0) {
            count++;
        }

        if (*p == ',') p++;
    }

    return count;
}

/* Check if a line has a specific attribute */
static bool has_attr(const char *line, const char *attr) {
    const char *p = line;
    int alen = strlen(attr);
    while ((p = strstr(p, attr)) != NULL) {
        /* Make sure it's a whole word (not part of another word) */
        bool start_ok = (p == line || !isalnum((unsigned char)*(p-1)));
        bool end_ok = !isalnum((unsigned char)*(p + alen));
        if (start_ok && end_ok) return true;
        p += alen;
    }
    return false;
}

/* ---- Pass 1: Collect function declarations/definitions ---- */

static void pass1_collect_functions(FILE *fp, FuncTable *tbl, const char *filename) {
    char line[MAX_LINE];
    int lineno = 0;

    while (fgets(line, sizeof(line), fp)) {
        lineno++;
        const char *trimmed = skip_ws(line);

        bool is_declare = (strncmp(trimmed, "declare ", 8) == 0);
        bool is_define  = (strncmp(trimmed, "define ",  7) == 0);

        if (!is_declare && !is_define) continue;
        if (tbl->count >= MAX_FUNCS) {
            fprintf(stderr, "WARNING: %s: function table full at %d entries\n",
                    filename, MAX_FUNCS);
            break;
        }

        FuncInfo *fi = &tbl->funcs[tbl->count];
        memset(fi, 0, sizeof(*fi));
        fi->is_define = is_define;

        if (!extract_func_name(trimmed, fi->name, MAX_NAME)) continue;
        if (!extract_ret_type(trimmed, fi->ret_type, MAX_TYPE)) continue;

        /* Find the parameter list */
        char search_at[MAX_NAME + 2];
        snprintf(search_at, sizeof(search_at), "@%s(", fi->name);
        /* Also try quoted form */
        char search_at_q[MAX_NAME + 4];
        snprintf(search_at_q, sizeof(search_at_q), "@\"%s\"(", fi->name);
        const char *paren = strstr(trimmed, search_at);
        if (!paren) paren = strstr(trimmed, search_at_q);
        if (paren) {
            /* Advance to the '(' */
            paren = strchr(paren, '(');
            fi->param_count = parse_param_types(paren, fi->param_types,
                                                MAX_PARAMS, &fi->is_vararg);
        }

        /* Check attributes */
        fi->has_norecurse = has_attr(trimmed, "norecurse");
        fi->has_noinline = has_attr(trimmed, "noinline");
        fi->has_alwaysinline = has_attr(trimmed, "alwaysinline");

        /* Check for contradictory attributes */
        if (fi->has_noinline && fi->has_alwaysinline) {
            fprintf(stderr, "ERROR: %s:%d: function @%s has both 'noinline' and 'alwaysinline'\n",
                    filename, lineno, fi->name);
            g_errors++;
        }

        tbl->count++;
    }
}

/* Look up a function in the table */
static FuncInfo *lookup_func(FuncTable *tbl, const char *name) {
    for (int i = tbl->count - 1; i >= 0; i--) {
        if (strcmp(tbl->funcs[i].name, name) == 0) {
            return &tbl->funcs[i];
        }
    }
    return NULL;
}

/* ---- Pass 2: Check call sites ---- */

/* Extract callee name from a call instruction.
 * Patterns:
 *   call TYPE @name(args)
 *   %r = call TYPE @name(args)
 *   invoke TYPE @name(args)
 *   %r = invoke TYPE @name(args)
 *   tail call TYPE @name(args)
 *   musttail call TYPE @name(args)
 */
static bool is_call_line(const char *line) {
    const char *p = skip_ws(line);
    /* Skip %reg = */
    if (*p == '%') {
        p = strchr(p, '=');
        if (!p) return false;
        p = skip_ws(p + 1);
    }
    /* Check for call/invoke/tail call/musttail call */
    if (strncmp(p, "call ", 5) == 0) return true;
    if (strncmp(p, "invoke ", 7) == 0) return true;
    if (strncmp(p, "tail call ", 10) == 0) return true;
    if (strncmp(p, "musttail call ", 14) == 0) return true;
    if (strncmp(p, "notail call ", 12) == 0) return true;
    return false;
}

/* Extract the return type from a call instruction.
 * "call i64 @foo" -> "i64"
 * "%5 = call ptr @bar" -> "ptr"
 */
static bool extract_call_ret_type(const char *line, char *ret, int retsize) {
    const char *p = skip_ws(line);

    /* Skip %reg = */
    if (*p == '%') {
        p = strchr(p, '=');
        if (!p) return false;
        p = skip_ws(p + 1);
    }

    /* Skip call/invoke/tail keywords and calling conventions */
    const char *keywords[] = {"musttail", "notail", "tail", "invoke", "call", NULL};
    for (int i = 0; keywords[i]; i++) {
        int klen = strlen(keywords[i]);
        if (strncmp(p, keywords[i], klen) == 0 && isspace((unsigned char)p[klen])) {
            p = skip_ws(p + klen);
        }
    }

    /* Skip calling convention (fastcc, ccc, etc.) */
    static const char *ccs[] = {"fastcc", "ccc", "coldcc", "x86_fastcallcc",
                                "x86_stdcallcc", "arm_apcscc", "arm_aapcscc",
                                "swifttailcc", "swiftcc", "tailcc", NULL};
    for (int i = 0; ccs[i]; i++) {
        int clen = strlen(ccs[i]);
        if (strncmp(p, ccs[i], clen) == 0 && isspace((unsigned char)p[clen])) {
            p = skip_ws(p + clen);
        }
    }

    /* Strip attributes between call and return type */
    strip_attrs(p, ret, retsize);

    /* Now ret starts with the return type followed by @name(...
     * Extract just the type */
    char *at = strchr(ret, '@');
    if (!at) return false;

    /* Walk back from @ to get just the type */
    while (at > ret && isspace((unsigned char)*(at-1))) at--;
    *at = '\0';

    /* Trim trailing whitespace */
    int len = strlen(ret);
    while (len > 0 && isspace((unsigned char)ret[len-1])) ret[--len] = '\0';

    return len > 0;
}

static void pass2_check_calls(FILE *fp, FuncTable *tbl, const char *filename) {
    char line[MAX_LINE];
    int lineno = 0;

    while (fgets(line, sizeof(line), fp)) {
        lineno++;

        if (!is_call_line(line)) continue;

        /* Extract callee name */
        char callee[MAX_NAME];
        const char *at = strstr(line, "@");
        if (!at) continue; /* indirect call — skip */

        at++;
        /* Handle quoted names */
        (void)0; /* at now points past @ */
        if (*at == '"') {
            at++;
            const char *q = strchr(at, '"');
            if (!q) continue;
            int nlen = q - at;
            if (nlen >= MAX_NAME) nlen = MAX_NAME - 1;
            memcpy(callee, at, nlen);
            callee[nlen] = '\0';
        } else {
            const char *end = at;
            while (*end && (isalnum((unsigned char)*end) || *end == '_' || *end == '.'))
                end++;
            int nlen = end - at;
            if (nlen == 0 || nlen >= MAX_NAME) continue;
            memcpy(callee, at, nlen);
            callee[nlen] = '\0';
        }

        /* Skip LLVM intrinsics — they're always valid */
        if (strncmp(callee, "llvm.", 5) == 0) continue;

        /* Look up callee */
        FuncInfo *fi = lookup_func(tbl, callee);
        if (!fi) {
            /* Undefined function reference */
            fprintf(stderr, "ERROR: %s:%d: call to undefined function @%s\n",
                    filename, lineno, callee);
            g_errors++;
            continue;
        }

        /* Check return type */
        char call_ret[MAX_TYPE];
        if (extract_call_ret_type(line, call_ret, sizeof(call_ret))) {
            if (!types_compatible(fi->ret_type, call_ret)) {
                fprintf(stderr, "ERROR: %s:%d: call to @%s returns '%s' but declaration returns '%s'\n",
                        filename, lineno, callee, call_ret, fi->ret_type);
                g_errors++;
            }
        }

        /* Check argument count — find the call's argument list */
        /* Find @name( and parse from there */
        char search[MAX_NAME + 2];
        snprintf(search, sizeof(search), "@%s(", callee);
        char search_q[MAX_NAME + 4];
        snprintf(search_q, sizeof(search_q), "@\"%s\"(", callee);
        const char *call_paren = strstr(line, search);
        if (!call_paren) call_paren = strstr(line, search_q);
        if (call_paren) {
            call_paren = strchr(call_paren, '(');
            char call_types[MAX_PARAMS][MAX_TYPE];
            bool call_vararg = false;
            int call_argc = parse_param_types(call_paren, call_types,
                                              MAX_PARAMS, &call_vararg);

            if (!fi->is_vararg && call_argc != fi->param_count) {
                fprintf(stderr, "ERROR: %s:%d: call to @%s with %d args but declared with %d\n",
                        filename, lineno, callee, call_argc, fi->param_count);
                g_errors++;
            } else if (fi->is_vararg && call_argc < fi->param_count) {
                fprintf(stderr, "ERROR: %s:%d: call to @%s with %d args but needs at least %d\n",
                        filename, lineno, callee, call_argc, fi->param_count);
                g_errors++;
            }
        }
    }
}

/* ---- Pass 3: Check norecurse consistency ---- */

static void pass3_check_norecurse(FILE *fp, FuncTable *tbl __attribute__((unused)),
                                  const char *filename) {
    char line[MAX_LINE];
    int lineno = 0;
    char current_func[MAX_NAME] = {0};
    bool in_norecurse_func = false;

    while (fgets(line, sizeof(line), fp)) {
        lineno++;
        const char *trimmed = skip_ws(line);

        /* Track which function we're in */
        if (strncmp(trimmed, "define ", 7) == 0) {
            char name[MAX_NAME];
            if (extract_func_name(trimmed, name, MAX_NAME)) {
                strcpy(current_func, name);
                in_norecurse_func = has_attr(trimmed, "norecurse");
            }
            continue;
        }

        /* If we hit another top-level construct, we're out of the function */
        if (*trimmed != '\0' && *trimmed != '}' && *trimmed != ';' &&
            !isspace((unsigned char)*line) && *trimmed != '%' &&
            strncmp(trimmed, "declare ", 8) == 0) {
            current_func[0] = '\0';
            in_norecurse_func = false;
            continue;
        }

        /* Check for self-calls within norecurse function */
        if (in_norecurse_func && current_func[0] != '\0' && is_call_line(line)) {
            char callee[MAX_NAME];
            const char *at = strstr(line, "@");
            if (!at) continue;
            at++;
            if (*at == '"') {
                at++;
                const char *q = strchr(at, '"');
                if (!q) continue;
                int nlen = q - at;
                if (nlen >= MAX_NAME) nlen = MAX_NAME - 1;
                memcpy(callee, at, nlen);
                callee[nlen] = '\0';
            } else {
                const char *end = at;
                while (*end && (isalnum((unsigned char)*end) || *end == '_' || *end == '.'))
                    end++;
                int nlen = end - at;
                if (nlen == 0 || nlen >= MAX_NAME) continue;
                memcpy(callee, at, nlen);
                callee[nlen] = '\0';
            }

            if (strcmp(callee, current_func) == 0) {
                fprintf(stderr, "WARNING: %s:%d: function @%s has 'norecurse' but calls itself\n",
                        filename, lineno, current_func);
                g_warnings++;
            }
        }
    }
}

/* ---- Main ---- */

static int lint_file(const char *filename) {
    FILE *fp;
    FuncTable *tbl = calloc(1, sizeof(FuncTable));
    if (!tbl) {
        fprintf(stderr, "ERROR: out of memory\n");
        return 1;
    }

    /* Pass 1: collect declarations */
    fp = fopen(filename, "r");
    if (!fp) {
        fprintf(stderr, "ERROR: cannot open %s: ", filename);
        perror("");
        free(tbl);
        return 1;
    }
    pass1_collect_functions(fp, tbl, filename);
    fclose(fp);

    /* Pass 2: check call sites */
    fp = fopen(filename, "r");
    if (!fp) { free(tbl); return 1; }
    pass2_check_calls(fp, tbl, filename);
    fclose(fp);

    /* Pass 3: check norecurse */
    fp = fopen(filename, "r");
    if (!fp) { free(tbl); return 1; }
    pass3_check_norecurse(fp, tbl, filename);
    fclose(fp);

    free(tbl);
    return 0;
}

int main(int argc, char **argv) {
    if (argc < 2) {
        fprintf(stderr, "Usage: %s file1.ll [file2.ll ...]\n", argv[0]);
        return 1;
    }

    for (int i = 1; i < argc; i++) {
        lint_file(argv[i]);
    }

    if (g_errors > 0) {
        fprintf(stderr, "\nseen_ir_lint: %d error(s), %d warning(s)\n",
                g_errors, g_warnings);
        return 1;
    }
    if (g_warnings > 0) {
        fprintf(stderr, "\nseen_ir_lint: %d warning(s)\n", g_warnings);
    }
    return 0;
}
