//! Process spawning and management

use std::process::{Command, Stdio, Child, Output};
use std::time::Duration;
use std::sync::{Arc, Mutex};
use std::thread;
use seen_common::{SeenResult, SeenError};

/// Exit status of a process
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExitStatus {
    code: Option<i32>,
    signal: Option<i32>,
}

impl ExitStatus {
    /// Create a new exit status
    pub fn new(code: Option<i32>, signal: Option<i32>) -> Self {
        Self { code, signal }
    }
    
    /// Get the exit code
    pub fn code(&self) -> Option<i32> {
        self.code
    }
    
    /// Check if the process exited successfully
    pub fn success(&self) -> bool {
        self.code == Some(0)
    }
    
    /// Get the signal that terminated the process (Unix only)
    pub fn signal(&self) -> Option<i32> {
        self.signal
    }
}

/// A running process
pub struct Process {
    child: Child,
    timeout: Option<Duration>,
}

impl Process {
    /// Wait for the process to complete
    pub fn wait(mut self) -> SeenResult<ExitStatus> {
        match self.timeout {
            Some(timeout) => self.wait_with_timeout(timeout),
            None => {
                let status = self.child.wait()
                    .map_err(|e| SeenError::runtime_error(format!("Failed to wait for process: {}", e)))?;
                
                Ok(ExitStatus::new(status.code(), None))
            }
        }
    }
    
    /// Wait for the process with a timeout
    fn wait_with_timeout(mut self, timeout: Duration) -> SeenResult<ExitStatus> {
        let pid = self.child.id();
        let handle = Arc::new(Mutex::new(Some(self.child)));
        let handle_clone = Arc::clone(&handle);
        
        // Spawn a thread to wait for the process
        let waiter = thread::spawn(move || {
            let mut child = handle_clone.lock().unwrap().take().unwrap();
            child.wait()
        });
        
        // Wait for either completion or timeout
        thread::sleep(timeout);
        
        if waiter.is_finished() {
            match waiter.join() {
                Ok(Ok(status)) => Ok(ExitStatus::new(status.code(), None)),
                Ok(Err(e)) => Err(SeenError::runtime_error(format!("Process wait failed: {}", e))),
                Err(_) => Err(SeenError::runtime_error("Process wait thread panicked")),
            }
        } else {
            // Timeout occurred, kill the process
            #[cfg(unix)]
            {
                use nix::sys::signal::{kill, Signal};
                use nix::unistd::Pid;
                
                let _ = kill(Pid::from_raw(pid as i32), Signal::SIGTERM);
                thread::sleep(Duration::from_millis(100));
                let _ = kill(Pid::from_raw(pid as i32), Signal::SIGKILL);
            }
            
            #[cfg(windows)]
            {
                let mut handle = handle.lock().unwrap();
                if let Some(ref mut child) = *handle {
                    let _ = child.kill();
                }
            }
            
            Err(SeenError::runtime_error(format!("Process timed out after {:?}", timeout)))
        }
    }
    
    /// Get the process ID
    pub fn id(&self) -> u32 {
        self.child.id()
    }
    
    /// Kill the process
    pub fn kill(mut self) -> SeenResult<()> {
        self.child.kill()
            .map_err(|e| SeenError::runtime_error(format!("Failed to kill process: {}", e)))
    }
    
    /// Get mutable access to stdin
    pub fn stdin(&mut self) -> Option<&mut std::process::ChildStdin> {
        self.child.stdin.as_mut()
    }
    
    /// Get mutable access to stdout
    pub fn stdout(&mut self) -> Option<&mut std::process::ChildStdout> {
        self.child.stdout.as_mut()
    }
    
    /// Get mutable access to stderr
    pub fn stderr(&mut self) -> Option<&mut std::process::ChildStderr> {
        self.child.stderr.as_mut()
    }
}

/// Builder for spawning processes
pub struct ProcessBuilder {
    command: String,
    args: Vec<String>,
    env: Vec<(String, String)>,
    working_dir: Option<String>,
    stdin: Stdio,
    stdout: Stdio,
    stderr: Stdio,
    timeout: Option<Duration>,
}

impl ProcessBuilder {
    /// Create a new process builder
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            args: Vec::new(),
            env: Vec::new(),
            working_dir: None,
            stdin: Stdio::inherit(),
            stdout: Stdio::inherit(),
            stderr: Stdio::inherit(),
            timeout: None,
        }
    }
    
    /// Add an argument
    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }
    
    /// Add multiple arguments
    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.args.extend(args.into_iter().map(|s| s.into()));
        self
    }
    
    /// Set an environment variable
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.push((key.into(), value.into()));
        self
    }
    
    /// Set the working directory
    pub fn current_dir(mut self, dir: impl Into<String>) -> Self {
        self.working_dir = Some(dir.into());
        self
    }
    
    /// Redirect stdin
    pub fn stdin(mut self, stdin: Stdio) -> Self {
        self.stdin = stdin;
        self
    }
    
    /// Redirect stdout
    pub fn stdout(mut self, stdout: Stdio) -> Self {
        self.stdout = stdout;
        self
    }
    
    /// Redirect stderr
    pub fn stderr(mut self, stderr: Stdio) -> Self {
        self.stderr = stderr;
        self
    }
    
    /// Set a timeout for the process
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }
    
    /// Spawn the process
    pub fn spawn(self) -> SeenResult<Process> {
        let mut cmd = Command::new(&self.command);
        cmd.args(&self.args)
            .stdin(self.stdin)
            .stdout(self.stdout)
            .stderr(self.stderr);
        
        for (key, value) in self.env {
            cmd.env(key, value);
        }
        
        if let Some(dir) = self.working_dir {
            cmd.current_dir(dir);
        }
        
        let child = cmd.spawn()
            .map_err(|e| SeenError::runtime_error(format!("Failed to spawn process '{}': {}", self.command, e)))?;
        
        Ok(Process {
            child,
            timeout: self.timeout,
        })
    }
    
    /// Run the process and wait for completion
    pub fn run(self) -> SeenResult<ExitStatus> {
        let process = self.spawn()?;
        process.wait()
    }
    
    /// Run the process and capture output
    pub fn output(self) -> SeenResult<Output> {
        let mut cmd = Command::new(&self.command);
        cmd.args(&self.args);
        
        for (key, value) in self.env {
            cmd.env(key, value);
        }
        
        if let Some(dir) = self.working_dir {
            cmd.current_dir(dir);
        }
        
        let output = cmd.output()
            .map_err(|e| SeenError::runtime_error(format!("Failed to run process '{}': {}", self.command, e)))?;
        
        Ok(output)
    }
}

/// Set up signal handlers for proper process management
pub fn setup_signal_handlers() -> SeenResult<()> {
    #[cfg(unix)]
    {
        use nix::sys::signal::{self, Signal, SigHandler};
        
        // Ignore SIGPIPE to prevent crashes when writing to closed pipes
        unsafe {
            signal::signal(Signal::SIGPIPE, SigHandler::SigIgn)
                .map_err(|e| SeenError::runtime_error(format!("Failed to set up signal handler: {}", e)))?;
        }
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_process_builder_creation() {
        let builder = ProcessBuilder::new("echo")
            .arg("hello")
            .arg("world")
            .env("TEST_VAR", "test_value");
        
        assert_eq!(builder.command, "echo");
        assert_eq!(builder.args, vec!["hello", "world"]);
        assert_eq!(builder.env, vec![("TEST_VAR".to_string(), "test_value".to_string())]);
    }
    
    #[test]
    fn test_exit_status() {
        let status = ExitStatus::new(Some(0), None);
        assert!(status.success());
        assert_eq!(status.code(), Some(0));
        
        let failed = ExitStatus::new(Some(1), None);
        assert!(!failed.success());
        assert_eq!(failed.code(), Some(1));
    }
}