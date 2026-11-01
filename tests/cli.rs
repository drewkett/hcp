//! Runs the hcp binary end to end against a local mock of the ping server

use std::{
    io::{BufRead, BufReader, Read, Write},
    net::TcpListener,
    process::{Command, Output, Stdio},
    sync::{Arc, Mutex},
    thread,
};

const UUID: &str = "12345678-1234-1234-1234-123456789abc";
const NO_SUCH_COMMAND: &str = "hcp-test-no-such-command";

fn start_path() -> String {
    format!("/{}/start", UUID)
}

fn finish_path() -> String {
    format!("/{}", UUID)
}

fn fail_path() -> String {
    format!("/{}/fail", UUID)
}

#[derive(Debug, Clone)]
struct Request {
    method: String,
    path: String,
    body: String,
}

struct MockServer {
    url: String,
    requests: Arc<Mutex<Vec<Request>>>,
}

impl MockServer {
    /// Starts a server which responds to requests with the given statuses in
    /// order and with 200 once they run out
    fn start(statuses: &[u16]) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let recorded = requests.clone();
        let statuses = statuses.to_vec();
        thread::spawn(move || {
            let mut statuses = statuses.into_iter();
            for stream in listener.incoming() {
                let mut stream = stream.unwrap();
                let request = read_request(&mut stream);
                recorded.lock().unwrap().push(request);
                let status = statuses.next().unwrap_or(200);
                write!(
                    stream,
                    "HTTP/1.1 {} Mock\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    status
                )
                .unwrap();
            }
        });
        Self { url, requests }
    }

    fn requests(&self) -> Vec<Request> {
        self.requests.lock().unwrap().clone()
    }

    fn paths(&self) -> Vec<String> {
        self.requests().into_iter().map(|r| r.path).collect()
    }

    /// The body of the last request received
    fn last_body(&self) -> String {
        self.requests().last().unwrap().body.clone()
    }

    fn hcp(&self) -> Command {
        let mut cmd = bare_hcp();
        cmd.args(["--hcp-id", UUID, "--hcp-url", &self.url]);
        cmd
    }

    fn run(&self, args: &[&str]) -> Output {
        self.hcp().args(args).output().unwrap()
    }
}

/// A command for hcp that isn't affected by the environment running the tests
fn bare_hcp() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_hcp"));
    for var in [
        "HCP_ID",
        "HCP_URL",
        "HCP_TEE",
        "HCP_IGNORE_CODE",
        "ALL_PROXY",
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "all_proxy",
        "http_proxy",
        "https_proxy",
    ] {
        cmd.env_remove(var);
    }
    cmd.stdin(Stdio::null());
    cmd
}

fn read_request(stream: &mut impl Read) -> Request {
    let mut rdr = BufReader::new(stream);
    let mut line = String::new();
    rdr.read_line(&mut line).unwrap();
    let mut request_line = line.split_whitespace();
    let method = request_line.next().unwrap().to_string();
    let path = request_line.next().unwrap().to_string();
    let mut content_length = 0;
    loop {
        line.clear();
        rdr.read_line(&mut line).unwrap();
        let header = line.trim();
        if header.is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':')
            && name.eq_ignore_ascii_case("content-length")
        {
            content_length = value.trim().parse().unwrap();
        }
    }
    let mut body = vec![0; content_length];
    rdr.read_exact(&mut body).unwrap();
    Request {
        method,
        path,
        body: String::from_utf8_lossy(&body).into_owned(),
    }
}

#[test]
fn no_command_pings_success_without_start() {
    let server = MockServer::start(&[]);
    let output = server.run(&[]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(server.paths(), [finish_path()]);
    assert_eq!(server.last_body(), "No command given");
}

#[test]
fn spawn_failure_pings_fail() {
    let server = MockServer::start(&[]);
    let output = server.run(&[NO_SUCH_COMMAND]);
    assert_eq!(output.status.code(), Some(193));
    assert_eq!(server.paths(), [start_path(), fail_path()]);
    assert!(server.last_body().contains("Failed to spawn process"));
}

#[test]
fn ping_methods() {
    let server = MockServer::start(&[]);
    server.run(&[NO_SUCH_COMMAND]);
    let methods: Vec<_> = server.requests().into_iter().map(|r| r.method).collect();
    assert_eq!(methods, ["GET", "POST"]);
}

#[test]
fn url_can_come_from_env_and_have_trailing_slash() {
    let server = MockServer::start(&[]);
    let output = bare_hcp()
        .env("HCP_ID", UUID)
        .env("HCP_URL", format!("{}/ping/", server.url))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(server.paths(), [format!("/ping/{}", UUID)]);
}

#[test]
fn invalid_url_is_rejected() {
    let output = bare_hcp()
        .args(["--hcp-id", UUID, "--hcp-url", "hc.example.com"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("hc.example.com"));
}

#[test]
fn server_error_on_start_is_retried() {
    let server = MockServer::start(&[500]);
    let output = server.run(&[NO_SUCH_COMMAND]);
    assert_eq!(output.status.code(), Some(193));
    assert_eq!(server.paths(), [start_path(), start_path(), fail_path()]);
}

#[test]
fn client_error_on_start_exits_without_retrying() {
    let server = MockServer::start(&[404]);
    let output = server.run(&[NO_SUCH_COMMAND]);
    assert_eq!(output.status.code(), Some(195));
    assert_eq!(server.paths(), [start_path()]);
}

#[cfg(unix)]
mod unix {
    use super::*;
    use std::{
        fs,
        time::{Duration, Instant},
    };

    #[test]
    fn success_pings_finish_with_output() {
        let server = MockServer::start(&[]);
        let output = server.run(&["sh", "-c", "echo to-stdout; echo to-stderr >&2"]);
        assert_eq!(output.status.code(), Some(0));
        assert_eq!(server.paths(), [start_path(), finish_path()]);
        let body = server.last_body();
        assert!(body.contains("exit code 0"), "{}", body);
        assert!(body.contains("stdout:\nto-stdout\n"), "{}", body);
        assert!(body.contains("stderr:\nto-stderr\n"), "{}", body);
        assert!(output.stdout.is_empty());
        assert!(output.stderr.is_empty());
    }

    #[test]
    fn failure_pings_fail_and_forwards_code() {
        let server = MockServer::start(&[]);
        let output = server.run(&["sh", "-c", "exit 3"]);
        assert_eq!(output.status.code(), Some(3));
        assert_eq!(server.paths(), [start_path(), fail_path()]);
        assert!(server.last_body().contains("exit code 3"));
    }

    #[test]
    fn ignore_code_pings_finish_and_exits_zero() {
        let server = MockServer::start(&[]);
        let output = server.run(&["--hcp-ignore-code", "sh", "-c", "exit 3"]);
        assert_eq!(output.status.code(), Some(0));
        assert_eq!(server.paths(), [start_path(), finish_path()]);
        assert!(server.last_body().contains("exit code 3"));
    }

    #[test]
    fn tee_copies_output_locally() {
        let server = MockServer::start(&[]);
        let output = server.run(&[
            "--hcp-tee",
            "sh",
            "-c",
            "echo to-stdout; echo to-stderr >&2",
        ]);
        assert_eq!(output.stdout, b"to-stdout\n");
        assert_eq!(output.stderr, b"to-stderr\n");
        assert!(server.last_body().contains("to-stdout"));
    }

    #[test]
    fn hcp_env_is_stripped_from_child() {
        let server = MockServer::start(&[]);
        let output = server
            .hcp()
            .env("HCP_ID", UUID)
            .env("HCP_URL", &server.url)
            .env("HCP_TEE", "1")
            .env("HCP_IGNORE_CODE", "1")
            .env("HCP_TEST_OTHER", "kept")
            .args(["sh", "-c", "env"])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(0));
        let body = server.last_body();
        assert!(body.contains("HCP_TEST_OTHER=kept"), "{}", body);
        for var in ["HCP_ID", "HCP_URL", "HCP_TEE", "HCP_IGNORE_CODE"] {
            assert!(!body.contains(var), "{}", body);
        }
    }

    #[test]
    fn server_error_on_finish_is_retried() {
        let server = MockServer::start(&[200, 503]);
        let output = server.run(&["sh", "-c", "exit 0"]);
        assert_eq!(output.status.code(), Some(0));
        assert_eq!(server.paths(), [start_path(), finish_path(), finish_path()]);
    }

    #[test]
    fn client_error_on_finish_exits_with_http_code() {
        let server = MockServer::start(&[200, 404]);
        let output = server.run(&["sh", "-c", "exit 3"]);
        assert_eq!(output.status.code(), Some(195));
        assert_eq!(server.paths(), [start_path(), fail_path()]);
    }

    #[test]
    fn sigterm_is_forwarded_to_child() {
        let server = MockServer::start(&[]);
        let marker = std::env::temp_dir().join(format!("hcp-test-{}", std::process::id()));
        let _ = fs::remove_file(&marker);
        let child = server
            .hcp()
            .args(["sh", "-c", "touch \"$0\"; exec sleep 30"])
            .arg(&marker)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        // Before the child is running, the signal would make hcp itself exit
        let started = Instant::now();
        while !marker.exists() {
            assert!(started.elapsed() < Duration::from_secs(10));
            thread::sleep(Duration::from_millis(10));
        }
        fs::remove_file(&marker).unwrap();
        let killed = Command::new("kill")
            .args(["-TERM", &child.id().to_string()])
            .status()
            .unwrap();
        assert!(killed.success());
        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(196));
        assert_eq!(server.paths(), [start_path(), fail_path()]);
        assert!(server.last_body().contains("without an exit code"));
    }
}
