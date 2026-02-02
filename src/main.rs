use bstr::ByteSlice;
use clap::Parser;
use std::{
    collections::VecDeque,
    ffi::OsString,
    fmt::Write,
    process::{exit, Command, Stdio},
    time::Duration,
};

const EXIT_HCP_SPAWN: i32 = 961;
const EXIT_HCP_IO: i32 = 962;
const EXIT_HCP_HTTP: i32 = 963;
const EXIT_HCP_UNKNOWN: i32 = 964;
const TEE_MAX_BYTES: usize = 40_000;

/// Trims everything after the last '\r' or '\n'
fn trim_trailing(buf: &[u8]) -> &[u8] {
    buf.iter()
        .rev()
        .position(|&c| c == b'\n' || c == b'\r')
        .map(|i_end| buf.split_at(buf.len() - i_end).0)
        .unwrap_or_default()
}

/// This reads the rdr to the end, copies the data to wrtr and returns the last
/// TEE_MAX_BYTES of data as a Vec
fn tee(mut rdr: impl std::io::Read, mut wrtr: impl std::io::Write, max_bytes: usize) -> std::io::Result<Vec<u8>> {
    let mut tail = VecDeque::new();
    let mut write_buf = Vec::new();
    let mut buf = [0; 16 * 1024];
    loop {
        match rdr.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                let read_contents = &buf[..n];
                tail.extend(read_contents);
                if tail.len() > max_bytes {
                    let excess = tail.len() - max_bytes;
                    tail.drain(..excess);
                }
                write_buf.extend_from_slice(read_contents);
                // Only write contents up to last new line. Since both stdout and
                // stderr can be writing at the same time, attempt to line buffer
                // to make output look nicer
                let to_write = trim_trailing(&write_buf);
                if !to_write.is_empty() {
                    wrtr.write_all(to_write)?;
                    let n_written = to_write.len();
                    write_buf.drain(..n_written);
                }
            }
            Err(e) => return Err(e),
        }
    }
    if !write_buf.is_empty() {
        wrtr.write_all(&write_buf)?;
    }
    Ok(tail.into())
}

/// Run a subprocess and ping healthchecks.io with the result
#[derive(Parser)]
#[command(name = "hcp", version, trailing_var_arg = true)]
struct Args {
    /// Sets the healthchecks id
    #[arg(long = "hcp-id", env = "HCP_ID")]
    hcp_id: Option<String>,

    /// Also output cmd stdout/stderr to local stdout/stderr
    #[arg(long = "hcp-tee", env = "HCP_TEE")]
    hcp_tee: bool,

    /// Ignore the return code from cmd
    #[arg(long = "hcp-ignore-code", env = "HCP_IGNORE_CODE")]
    hcp_ignore_code: bool,

    /// Command and arguments to run
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    cmd: Vec<OsString>,
}

fn make_agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(10)))
        .timeout_recv_body(Some(Duration::from_secs(10)))
        .timeout_send_body(Some(Duration::from_secs(10)))
        .build()
        .new_agent()
}

mod internal {
    use super::{EXIT_HCP_HTTP, make_agent};
    use std::{process::exit, thread, time::Duration};
    use ureq::Agent;

    /// Check if buf is only valid hex characters
    fn is_hex(buf: &[u8]) -> bool {
        buf.iter()
            .all(|b| matches!(b, b'0'..=b'9'|b'a'..=b'z'|b'A'..=b'Z'))
    }

    /// A Uuid newtype wrapper, which checks validity on creation and leaves the uuid stored
    /// as hex bytes
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct Uuid([u8; 36]);

    impl Uuid {
        pub fn from_str(s: &str) -> Option<Self> {
            if s.len() != 36 {
                return None;
            }
            let mut uuid = [0; 36];
            uuid.copy_from_slice(s.as_bytes());
            if is_hex(&uuid[..8])
                && uuid[8] == b'-'
                && is_hex(&uuid[9..13])
                && uuid[13] == b'-'
                && is_hex(&uuid[14..18])
                && uuid[18] == b'-'
                && is_hex(&uuid[19..23])
                && uuid[23] == b'-'
                && is_hex(&uuid[24..])
            {
                Some(Self(uuid))
            } else {
                None
            }
        }

        fn as_str(&self) -> &str {
            // SAFETY: Uuid can only be created with from_str and it checks for
            // valid utf-8 characters
            unsafe { std::str::from_utf8_unchecked(&self.0) }
        }
    }

    /// Returns true if the error is retryable (5xx or connection error)
    fn is_retryable(err: &ureq::Error) -> bool {
        match err {
            ureq::Error::StatusCode(code) => *code >= 500,
            _ => true,
        }
    }

    /// A wrapper struct to implement helper functions for pinging healthchecks.io
    pub struct HealthCheck {
        uuid: Uuid,
        agent: Agent,
    }

    impl HealthCheck {
        pub fn from_str(s: &str) -> Option<Self> {
            Uuid::from_str(s).map(|uuid| Self {
                uuid,
                agent: make_agent(),
            })
        }

        fn base_url(&self) -> String {
            let mut url = "https://hc-ping.com/".to_string();
            url.push_str(self.uuid.as_str());
            url
        }

        fn start_url(&self) -> String {
            let mut url = self.base_url();
            url.push_str("/start");
            url
        }

        fn finish_url(&self) -> String {
            self.base_url()
        }

        fn fail_url(&self) -> String {
            let mut url = self.base_url();
            url.push_str("/fail");
            url
        }

        pub fn start(&self) {
            let url = self.start_url();
            let result = self.agent.get(&url).call().or_else(|e| {
                if is_retryable(&e) {
                    eprintln!("Healthcheck /start failed, retrying in 2s: {}", e);
                    thread::sleep(Duration::from_secs(2));
                    self.agent.get(&url).call()
                } else {
                    Err(e)
                }
            });
            if let Err(e) = result {
                eprintln!("Error on healthchecks /start call: {}", e);
                exit(EXIT_HCP_HTTP)
            }
        }

        pub fn finish_and_exit(&self, msg: &str, code: i32, log: bool) -> ! {
            let url = if code == 0 {
                self.finish_url()
            } else {
                self.fail_url()
            };
            if log {
                eprintln!("{}", msg);
            }
            let result = self.agent.post(&url).send(msg).or_else(|e| {
                if is_retryable(&e) {
                    eprintln!("Healthcheck finish failed, retrying in 2s: {}", e);
                    thread::sleep(Duration::from_secs(2));
                    self.agent.post(&url).send(msg)
                } else {
                    Err(e)
                }
            });
            if let Err(e) = result {
                eprintln!("Error sending finishing request to healthchecks: {}", e);
                exit(EXIT_HCP_HTTP)
            }
            exit(code)
        }
    }

    #[cfg(test)]
    mod test {
        use super::*;

        #[test]
        fn test_uuid() {
            fn should_be_none(value: &str) {
                assert_eq!(Uuid::from_str(value), None)
            }

            fn should_be_some(value: &str) {
                assert_eq!(
                    Uuid::from_str(value).as_ref().map(|o| o.as_str()),
                    Some(value)
                )
            }
            should_be_some("abcdefgh-1234-5678-9012-ijklmnopqrst");
            should_be_some("ABCDEFGH-1234-5678-9012-ijklmnopqrst");
            should_be_none("ABCDEFGH-1234-5678-9012-ijklmnopqrstu");
            should_be_none("ABCDEFGH0123415678190121ijklmnopqrst");
            should_be_none("abcdef");
        }
    }
}

use internal::HealthCheck;

#[cfg(unix)]
mod signal {
    use std::sync::atomic::{AtomicI32, Ordering};

    pub static SIGNAL_RECEIVED: AtomicI32 = AtomicI32::new(0);

    extern "C" fn handler(sig: libc::c_int) {
        SIGNAL_RECEIVED.store(sig, Ordering::SeqCst);
    }

    pub fn install_handlers() {
        unsafe {
            libc::signal(libc::SIGTERM, handler as libc::sighandler_t);
            libc::signal(libc::SIGINT, handler as libc::sighandler_t);
        }
    }

    pub fn check_and_forward(child_pid: u32) {
        let sig = SIGNAL_RECEIVED.swap(0, Ordering::SeqCst);
        if sig != 0 {
            unsafe {
                libc::kill(child_pid as libc::pid_t, sig);
            }
        }
    }

    pub fn wait_or_kill(child: &mut std::process::Child) -> std::io::Result<std::process::ExitStatus> {
        let pid = child.id();
        // Check for pending signal before entering wait loop
        check_and_forward(pid);

        // Try waiting with periodic signal checks
        loop {
            match child.try_wait()? {
                Some(status) => return Ok(status),
                None => {
                    check_and_forward(pid);
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
            }
        }
    }
}

fn main() {
    #[cfg(unix)]
    signal::install_handlers();

    let args = Args::parse();
    let tee_output = args.hcp_tee;
    let ignore_code = args.hcp_ignore_code;
    let hc = match args.hcp_id.as_deref() {
        Some(hcp_id) => match HealthCheck::from_str(hcp_id) {
            Some(hc) => hc,
            None => {
                eprintln!("Healthcheck Id isn't a valid uuid '{}'", hcp_id);
                exit(1);
            }
        },
        None => {
            eprintln!("No Healthcheck Id given");
            exit(1);
        }
    };
    let mut cmd_args = args.cmd.into_iter();
    let cmd = match cmd_args.next() {
        Some(cmd) => cmd,
        None => hc.finish_and_exit("No command given", 0, true),
    };
    hc.start();
    let mut proc = match Command::new(cmd)
        .args(cmd_args)
        .env_remove("HCP_ID")
        .env_remove("HCP_TEE")
        .env_remove("HCP_IGNORE_CODE")
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(p) => p,
        Err(e) => hc.finish_and_exit(&format!("Failed to spawn process: {}", e), EXIT_HCP_SPAWN, true),
    };

    let child_stdout = proc.stdout.take().unwrap();
    let child_stderr = proc.stderr.take().unwrap();

    let pipe_stdout = if tee_output {
        Some(std::io::stdout())
    } else {
        None
    };
    let pipe_stderr = if tee_output {
        Some(std::io::stderr())
    } else {
        None
    };

    // Spawn threads for continuously reading from the child process's stdout and stderr. If
    // tee_output is enabled forward the output to the processes pipes
    let stdout_thread = std::thread::spawn(move || {
        if let Some(pipe_stdout) = pipe_stdout {
            tee(child_stdout, pipe_stdout, TEE_MAX_BYTES)
        } else {
            tee(child_stdout, std::io::sink(), TEE_MAX_BYTES)
        }
    });
    let stderr_thread = std::thread::spawn(move || {
        if let Some(pipe_stderr) = pipe_stderr {
            tee(child_stderr, pipe_stderr, TEE_MAX_BYTES)
        } else {
            tee(child_stderr, std::io::sink(), TEE_MAX_BYTES)
        }
    });

    #[cfg(unix)]
    let wait_result = signal::wait_or_kill(&mut proc);
    #[cfg(not(unix))]
    let wait_result = proc.wait();

    match wait_result {
        Ok(status) => {
            let out = match stdout_thread.join() {
                Ok(Ok(out)) => out,
                Ok(Err(e)) => hc.finish_and_exit(
                    &format!("Error reading stdout from child: {}", e),
                    EXIT_HCP_IO,
                    false,
                ),
                Err(e) => std::panic::resume_unwind(e),
            };
            let err = match stderr_thread.join() {
                Ok(Ok(err)) => err,
                Ok(Err(e)) => hc.finish_and_exit(
                    &format!("Error reading stderr from child: {}", e),
                    EXIT_HCP_IO,
                    false,
                ),
                Err(e) => std::panic::resume_unwind(e),
            };
            let mut msg = String::new();
            let mut code = match status.code() {
                Some(code) => {
                    if let Err(e) = writeln!(msg, "Command exited with exit code {}", code) {
                        eprintln!("Write to message buffer failed: {}", e)
                    }
                    code
                }
                None => {
                    msg.push_str("Command exited without an exit code\n");
                    EXIT_HCP_UNKNOWN
                }
            };
            if !out.is_empty() {
                let _ = writeln!(msg, "stdout:");
                let _ = writeln!(msg, "{}", out.as_bstr());
            }
            if !err.is_empty() {
                if !out.is_empty() {
                    let _ = writeln!(msg);
                }
                let _ = writeln!(msg, "stderr:");
                let _ = writeln!(msg, "{}", err.as_bstr());
            }
            if ignore_code {
                // 0 would indicate success
                code = 0;
            }
            hc.finish_and_exit(&msg, code, false)
        }
        Err(e) => {
            let msg = format!("Failed waiting for process: {}", e);
            hc.finish_and_exit(&msg, EXIT_HCP_IO, true)
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_trim_trailing() {
        assert_eq!(trim_trailing(b"abc\r\ncd"), b"abc\r\n");
        assert_eq!(trim_trailing(b"abc\r\nabc\ncd"), b"abc\r\nabc\n");
        assert_eq!(trim_trailing(b"abc"), b"");
    }

    #[test]
    fn test_tee() {
        fn run_test(input: &[u8]) {
            let input = input.to_vec();
            let rdr = std::io::Cursor::new(&input);
            let mut out_wrtr = Vec::new();
            let out_returned = tee(rdr, &mut out_wrtr, TEE_MAX_BYTES).unwrap();
            assert_eq!(input, out_wrtr);
            assert_eq!(input, out_returned);
        }
        run_test(b"abc\r\ncd\rfd");
        run_test(b"");
        run_test(b"abc");
    }

    #[test]
    fn test_tee_large_input_truncates() {
        let size = TEE_MAX_BYTES + 10_000;
        let input: Vec<u8> = (0..size).map(|i| (i % 256) as u8).collect();
        let rdr = std::io::Cursor::new(&input);
        let mut out_wrtr = Vec::new();
        let out_returned = tee(rdr, &mut out_wrtr, TEE_MAX_BYTES).unwrap();
        // Writer gets all data
        assert_eq!(input, out_wrtr);
        // Returned buffer is limited to TEE_MAX_BYTES (the tail)
        assert_eq!(out_returned.len(), TEE_MAX_BYTES);
        assert_eq!(out_returned, &input[size - TEE_MAX_BYTES..]);
    }
}
