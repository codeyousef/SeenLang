# Process

## Command Execution

```seen
import process
```

### Running Commands

| Function | Signature | Description |
|----------|-----------|-------------|
| `runCommand` | `(cmd: String) r: CommandResult` | Execute shell command, capture output |
| `runProgram` | `(path: String) r: Int` | Execute program, return exit code |
| `execCommand` | `(cmd: String) r: Int` | Execute command, return exit code |
| `runCommandOutput` | `(cmd: String) r: String` | Execute and return stdout |
| `runCommandOrAbort` | `(cmd: String) r: Void` | Execute, abort on failure |

### CommandResult

| Function | Description |
|----------|-------------|
| `commandWasSuccessful(result)` | Check if command succeeded |
| `commandOutput(result)` | Get stdout output |
| `isSuccess(result)` | Check exit code == 0 |

### Example

```seen
let result = runCommand("ls -la")
if commandWasSuccessful(result) {
    println(commandOutput(result))
}

let exitCode = runProgram("/usr/bin/gcc")
```

## Process Management

| Function | Signature | Description |
|----------|-----------|-------------|
| `processFork()` | `r: Int` | Fork process (returns child PID in parent, 0 in child) |
| `waitPid(pid: Int, flags: Int)` | `r: Int` | Wait for child (0=blocking, 1=WNOHANG) |
| `processExit(code: Int)` | `Void` | Terminate process |
| `getPid()` | `r: Int` | Get current process ID |

**Note:** Use `processFork()` instead of `fork()` to avoid shadowing POSIX `fork()`.

### Example

```seen
let pid = processFork()
if pid == 0 {
    // child process
    println("I'm the child")
    processExit(0)
} else {
    // parent process
    println("Child PID: {pid}")
    waitPid(pid, 0)
}
```

## Environment Variables

```seen
import env
```

| Function | Signature | Description |
|----------|-----------|-------------|
| `get` | `(name: String) r: String` | Get env var (empty if missing) |
| `has` | `(name: String) r: Bool` | Check if env var exists |
| `set` | `(name: String, value: String) r: Bool` | Set env var |
| `removeEnv` | `(name: String) r: Bool` | Remove env var |
| `tryGet` | `(name: String) r: Option<String>` | Get env var as Option |
| `getOrDefault` | `(name: String, default: String) r: String` | Get with fallback |

### Example

```seen
let home = get("HOME")
println("Home: {home}")

if has("DEBUG") {
    println("Debug mode enabled")
}

set("MY_VAR", "hello")
```

## Command-Line Arguments

```seen
let arguments = args()  // returns Array<String>
for arg in arguments {
    println(arg)
}
```

### Shell Quoting

```seen
let safe = shellQuote("file with spaces.txt")
// Returns: 'file with spaces.txt'
```

## Runtime Functions

| Function | Description |
|----------|-------------|
| `__ExecuteProgram(path)` | Execute program, return exit code |
| `__seen_fork()` | Fork process |
| `__seen_waitpid(pid, flags)` | Wait for child |
| `__seen_exit(code)` | Exit process |
| `__seen_getpid()` | Get PID |
| `__HasEnv(name)` | Check env var |
| `__GetEnv(name)` | Get env var |
| `__SetEnv(name, value)` | Set env var |
| `__RemoveEnv(name)` | Remove env var |
| `seen_runtime_init(argc, argv)` | Initialize runtime with args |
