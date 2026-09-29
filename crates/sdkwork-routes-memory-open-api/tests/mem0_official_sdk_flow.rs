//! The official **Python** `mem0ai` client driving the mem0 platform wire.
//!
//! This is the strongest evidence the compatibility surface can produce: it is
//! the published package talking to the production assembly over a real socket,
//! composing its own headers, parsing its own responses and raising its own
//! exception types. Nothing here is a stub or a recorded fixture.
//!
//! It is not a soft gate — run it explicitly and it fails loudly:
//!
//! ```text
//! MEM0_E2E_PYTHON=<interpreter with mem0ai> \
//!   cargo test -p sdkwork-routes-memory-open-api \
//!     --test mem0_official_sdk_flow -- --ignored --nocapture
//! ```
//!
//! Discovery order: `MEM0_E2E_PYTHON`, then the interpreter of an activated
//! `VIRTUAL_ENV`, then `<repo>/.venv`, then `python`, `python3` and `py`. The
//! first one that can `import mem0` wins.
//!
//! The venv locations are probed by name *and* by import because the two
//! disagree: `mem0ai` is normally installed into an isolated environment (see
//! the workspace runtime-isolation rules), which is not the interpreter that
//! bare `python` resolves to. Discovering by executable name alone therefore
//! loses the gate the moment the environment it was validated in is not the one
//! on `PATH` — the test still exists, still says `--ignored`, and quietly stops
//! being re-runnable. Probing the conventional in-tree `.venv` keeps
//! `--ignored` a one-command regression.
//!
//! The driver (`tests/mem0_official_sdk/acceptance.py`) reports what it observed
//! as JSON; the assertions live in `tests/mem0_official_sdk_support/mod.rs`,
//! shared with the JavaScript client's test, so a waiver granted to one client
//! cannot be quietly withheld from the other.

#[path = "mem0_official_sdk_support/mod.rs"]
mod support;

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use support::{
    assert_wire_observations, build_app, mem0_api_key, parse_result, ClientExpectations,
    BATCH_FIRST, BATCH_FIRST_UPDATED, BATCH_SECOND, BATCH_SECOND_UPDATED, CONVERSATION,
    EXPECTED_TENANT, UNKNOWN_ID, UPDATED_TEXT,
};

/// The refusals this client attempts, in call order.
///
/// Declared rather than counted: the shared asserter checks the recorded set
/// against this list, so a refusal that stops being attempted is a failure, not
/// a silently smaller number.
const REFUSALS: [&str; 6] = [
    "get_all(filters=user_id)",
    "get_all(page=2)",
    "delete_all(user_id=...)",
    // Python-only: this client's `delete_all(filters=...)` is the documented
    // scoped sweep, and the JS client's `DeleteAllMemoryOptions` has no `filters`
    // at all, so it cannot attempt this one. A shared list would test something
    // the other client has no way to send.
    "delete_all(filters=user_id)",
    "update(timestamp=...)",
    "delete(delete_linked=True)",
];

/// The boundary rejections this client expects, in call order.
const REJECTIONS: [(&str, &str, &str); 3] = [
    ("batch_update(unknown memory_id)", "HTTP_404", "MemoryNotFoundError"),
    (
        "batch_update(entry with no text or metadata)",
        "HTTP_400",
        "ValidationError",
    ),
    ("batch_update(1001 entries)", "HTTP_400", "ValidationError"),
];

/// Conventional interpreter paths inside a virtual environment.
///
/// Both layouts, because the same repository is driven from Windows and from
/// POSIX: `Scripts/python.exe` on the former, `bin/python` on the latter.
fn venv_interpreters(root: &Path) -> [PathBuf; 2] {
    [root.join("Scripts").join("python.exe"), root.join("bin").join("python")]
}

/// An interpreter that can import the official client, or a reason there is none.
///
/// Probing by importing is the only reliable test: the executable name carries
/// no information about which site-packages it sees, and a managed venv, a
/// `py`-launcher default, and a system install are all plausible answers on the
/// same machine.
fn find_python_with_mem0() -> Result<String, String> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(explicit) = std::env::var("MEM0_E2E_PYTHON") {
        if !explicit.trim().is_empty() {
            candidates.push(PathBuf::from(explicit));
        }
    }
    // An activated environment names itself; honour it before guessing.
    if let Ok(active) = std::env::var("VIRTUAL_ENV") {
        if !active.trim().is_empty() {
            candidates.extend(venv_interpreters(Path::new(&active)));
        }
    }
    // Then the conventional in-tree location (`python -m venv .venv`), which is
    // where this project's runtime-isolation rules put the isolated environment.
    // `ancestors` rather than `join("..")` so a miss reports a readable path
    // instead of one still carrying `..` segments.
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate directory is always two levels below the repository root");
    candidates.extend(venv_interpreters(&repo_root.join(".venv")));
    candidates.extend(
        ["python", "python3", "py"]
            .into_iter()
            .map(PathBuf::from),
    );

    let mut tried = Vec::new();
    for candidate in candidates {
        let probe = Command::new(&candidate)
            .args(["-c", "import mem0"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        match probe {
            Ok(status) if status.success() => return Ok(candidate.to_string_lossy().into_owned()),
            Ok(status) => tried.push(format!("{} (exit {status})", candidate.display())),
            Err(error) => tried.push(format!("{} ({error})", candidate.display())),
        }
    }
    Err(format!(
        "no interpreter could `import mem0`; tried {}. Install it with \
         `pip install mem0ai==2.2.0`, or point MEM0_E2E_PYTHON at the interpreter that has it.",
        tried.join(", ")
    ))
}

/// Runs the driver and returns `(stdout, stderr, exit code)`.
fn run_driver(python: &str, base_url: &str) -> (String, String, Option<i32>) {
    let script = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/mem0_official_sdk/acceptance.py"
    );
    let output = Command::new(python)
        .arg(script)
        .env("MEM0_BASE_URL", base_url)
        .env("MEM0_API_KEY", mem0_api_key())
        .env("MEM0_EXPECT_TENANT", EXPECTED_TENANT)
        .env("MEM0_E2E_CONVERSATION", CONVERSATION)
        .env("MEM0_E2E_UPDATED_TEXT", UPDATED_TEXT)
        // The batch fixtures are passed in for the same reason as the two above:
        // the expected values must be owned here, or the assertions below would
        // only be checking the driver against itself.
        .env("MEM0_E2E_BATCH_FIRST", BATCH_FIRST)
        .env("MEM0_E2E_BATCH_SECOND", BATCH_SECOND)
        .env("MEM0_E2E_BATCH_FIRST_UPDATED", BATCH_FIRST_UPDATED)
        .env("MEM0_E2E_BATCH_SECOND_UPDATED", BATCH_SECOND_UPDATED)
        .env("MEM0_E2E_UNKNOWN_ID", UNKNOWN_ID)
        // Keep the SDK's own config and anon-id files inside the build tree
        // rather than the operator's home directory.
        .env("MEM0_DIR", env!("CARGO_TARGET_TMPDIR"))
        .env("MEM0_TELEMETRY", "False")
        // The driver writes UTF-8 to stderr; a cp936 console must not turn a
        // diagnostic into an encoding crash.
        .env("PYTHONIOENCODING", "utf-8")
        // Do not leave `__pycache__` inside the repository.
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .output()
        .unwrap_or_else(|error| panic!("could not run `{python} {script}`: {error}"));

    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
        output.status.code(),
    )
}

#[tokio::test]
#[ignore = "binds a socket and needs the official mem0ai Python SDK; run with --ignored"]
async fn official_mem0_python_sdk_drives_the_platform_wire() {
    let _env = sdkwork_memory_test_support::web_auth::lock_integration_test_env().await;
    let app = build_app().await;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("an ephemeral loopback port binds");
    let address = listener.local_addr().expect("the bound address is known");
    let base_url = format!("http://{address}");

    // `TcpListener::bind` starts listening immediately, so the driver's very
    // first request is queued even if this task has not been polled yet.
    let server = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    let python = find_python_with_mem0().expect("an interpreter with the official mem0ai SDK");

    // `spawn_blocking` because this is a synchronous subprocess: awaiting the
    // handle leaves the executor free to drive the server task above.
    let served_url = base_url.clone();
    let interpreter = python.clone();
    let joined = tokio::task::spawn_blocking(move || run_driver(&interpreter, &served_url))
        .await
        .expect("the driver task does not panic");

    server.abort();

    let (stdout, stderr, exit_code) = joined;
    // Always surface the driver's narrative: it is the human-readable half of
    // the evidence, and on failure it is the only place the reason appears.
    println!("----- mem0ai Python driver ({python}) against {base_url} -----\n{stderr}");

    let result = parse_result(&stdout);

    let version = assert_wire_observations(
        &result,
        &stderr,
        "mem0ai Python",
        &ClientExpectations {
            refusals: &REFUSALS,
            // The six from the filter/paging list plus the feedback withdrawal,
            // which lives in its own block.
            refusals_total: 7,
            rejections: &REJECTIONS,
            // Three batch payloads plus the two post-delete read-backs.
            rejections_total: 5,
        },
    );

    // 17. Client-specific precision. The shared asserter can only require that
    //     this surface's wording *reaches* a client, because the two clients
    //     present failures differently (measured: this one parses the body's
    //     `detail` and passes it through bare; the JavaScript one keeps the raw
    //     body). Where the presentation *is* fixed by this client, pin the exact
    //     string — otherwise relaxing the shared assertion would have quietly
    //     cost us the precision.
    let observed = &result["observed"];
    assert_eq!(
        observed["get_after_delete"]["error"]["message"], "memory not found",
        "this client hands over `detail` verbatim: {observed}"
    );
    assert_eq!(
        observed["batch_delete_proof"][0]["error"]["message"], "memory not found",
        "{observed}"
    );
    assert_eq!(
        observed["rejections"][0]["error"]["message"],
        format!("memory {UNKNOWN_ID} not found"),
        "{observed}"
    );

    // 18. Client-specific capability: this client can patch `metadata` through
    //     `PUT /v1/batch/`, so the patch is asserted here. The JavaScript client
    //     cannot express it at all — it maps each entry to `{memory_id, text}`
    //     and drops every other field — so a shared assertion would be asserting
    //     something one of the two clients has no way to send.
    assert_eq!(
        observed["batch_update"]["second_metadata"], "second",
        "a metadata patch must reach the record: {observed}"
    );

    // 19. Client-specific: this client validates its own feedback enum *before*
    //     opening a socket, so the request never reached the wire and the
    //     server's closed-enum check is not covered by this run. Recorded to keep
    //     the two apart — the server-side check is pinned by `mem0_wire_flow.rs`,
    //     which does speak to the wire, and is deliberately not claimed here.
    //     The JavaScript client has no such guard, so its test asserts the
    //     server-side rejection instead.
    assert_eq!(
        observed["feedback_local_guard"]["type"], "ValueError",
        "{observed}"
    );
    assert!(
        observed["feedback_local_guard"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("POSITIVE")),
        "the local guard must state the accepted values: {observed}"
    );

    assert_eq!(exit_code, Some(0), "the driver exited {exit_code:?}\n{stderr}");

    println!(
        "official mem0ai {version} (Python) drove the platform wire end to end \
         ({} refusal(s), {} rejection(s) classified)\n",
        result["refusals_total"].as_u64().unwrap_or_default(),
        result["rejections_total"].as_u64().unwrap_or_default(),
    );
}
