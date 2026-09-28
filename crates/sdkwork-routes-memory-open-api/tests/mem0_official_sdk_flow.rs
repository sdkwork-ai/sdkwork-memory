//! Acceptance: the **official** `mem0ai` SDK drives the platform wire.
//!
//! Every other suite in this crate speaks to the router in-process. That is the
//! right shape for pinning behaviour — it is fast, hermetic, and it can assert
//! on interceptor internals. It is *not* evidence that a third-party client can
//! talk to the surface, because the client is then our own test code: it sets
//! exactly the headers we remembered to set, on the transport we chose, and
//! parses with our own expectations.
//!
//! This suite removes that assumption. It binds a real TCP listener, serves the
//! production assembly on it (`build_router_with_open_api` wrapped in the web
//! framework, over an in-memory store), and hands the base URL to the published
//! `mem0ai` Python package. `MemoryClient` then chooses its own endpoints, sets
//! its own headers — `Authorization: Token <key>`, `Mem0-User-ID`,
//! `X-Mem0-Client` — and raises its own exception classes. Nothing here
//! constructs a request by hand.
//!
//! `#[ignore]`: it needs a Python interpreter with `pip install mem0ai` and it
//! binds a port, so it cannot be part of a hermetic `cargo test --workspace`.
//! It is not a soft gate — run it explicitly and it fails loudly:
//!
//! ```text
//! MEM0_E2E_PYTHON=<interpreter with mem0ai> \
//!   cargo test -p sdkwork-routes-memory-open-api \
//!     --test mem0_official_sdk_flow -- --ignored --nocapture
//! ```
//!
//! Set `MEM0_E2E_PYTHON` to skip discovery; otherwise `python`, `python3`, and
//! `py` are probed in order and the first one that can `import mem0` is used.
//!
//! The driver (`tests/mem0_official_sdk/acceptance.py`) reports what it observed
//! as JSON; the assertions below re-derive every claim from that raw observation
//! rather than trusting the driver's own verdict. Both must agree.

use std::process::{Command, Stdio};

use axum::Router;
use sdkwork_intelligence_memory_service::OpenMemoryService;
use sdkwork_memory_test_support::web_auth::{lock_integration_test_env, memory_dev_api_key};
use sdkwork_routes_memory_open_api::{build_router_with_open_api, wrap_router_with_web_framework};
use sdkwork_web_core::DefaultWebRequestContextResolver;
use serde_json::Value;

const CONVERSATION: &str = "The operator prefers concise bullet-point summaries over prose.";
const UPDATED_TEXT: &str = "The operator prefers concise bullet-point summaries, not prose.";
/// Two subjects for the batch path. The updated forms differ from the originals,
/// which is what makes "the batch really wrote" observable after the fact — and
/// what lets this test assert the *expected* text rather than merely echoing what
/// the driver claims it sent.
const BATCH_FIRST: &str = "The staging database password rotates every ninety days.";
const BATCH_SECOND: &str = "The design review happens on the first Wednesday of the month.";
const BATCH_FIRST_UPDATED: &str = "The staging database password rotates every sixty days.";
const BATCH_SECOND_UPDATED: &str = "The design review happens on the first Thursday of the month.";
/// A `memory_id` that names nothing. A valid unsigned integer, so it is rejected
/// for being *absent* rather than for being malformed — the two failures are
/// different code paths and only the first is being exercised here.
const UNKNOWN_ID: &str = "999999999999999999";
const EXPECTED_TENANT: &str = "100001";
const RESULT_PREFIX: &str = "MEM0_E2E_RESULT ";

/// The raw claim-form credential — **without** an authorization scheme.
///
/// `MemoryClient` composes the header itself (`_client_headers` →
/// `Authorization: Token {api_key}`), so passing a value that already carries
/// the scheme sends `Token Token <claims>`. The credential bridge splits on the
/// first space, so that arrives at the claim parser as a key literally named
/// `Token api_key_id` and fails as a *missing claim* — a 401 whose message names
/// the wrong thing. The published contract is a bare key; give the client that.
fn mem0_api_key() -> String {
    memory_dev_api_key("2001", "mem0-compat-key")
}

async fn build_app() -> Router {
    let store = sdkwork_memory_test_support::space_fixtures::new_seeded_in_memory_store().await;
    wrap_router_with_web_framework(
        DefaultWebRequestContextResolver::default(),
        build_router_with_open_api(OpenMemoryService::new(store)),
    )
}

/// An interpreter that can import the official client, or a reason there is none.
///
/// Probing by importing is the only reliable test: the executable name carries
/// no information about which site-packages it sees, and a managed venv, a
/// `py`-launcher default, and a system install are all plausible answers on the
/// same machine.
fn find_python_with_mem0() -> Result<String, String> {
    let mut candidates: Vec<String> = Vec::new();
    if let Ok(explicit) = std::env::var("MEM0_E2E_PYTHON") {
        if !explicit.trim().is_empty() {
            candidates.push(explicit);
        }
    }
    candidates.extend(["python".to_owned(), "python3".to_owned(), "py".to_owned()]);

    let mut tried = Vec::new();
    for candidate in candidates {
        let probe = Command::new(&candidate)
            .args(["-c", "import mem0"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        match probe {
            Ok(status) if status.success() => return Ok(candidate),
            Ok(status) => tried.push(format!("{candidate} (exit {status})")),
            Err(error) => tried.push(format!("{candidate} ({error})")),
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

fn parse_result(stdout: &str) -> Value {
    let line = stdout
        .lines()
        .rev()
        .find(|line| line.starts_with(RESULT_PREFIX))
        .unwrap_or_else(|| panic!("the driver printed no `{RESULT_PREFIX}` line; stdout was:\n{stdout}"));
    serde_json::from_str(line.trim_start_matches(RESULT_PREFIX))
        .unwrap_or_else(|error| panic!("the driver's result line is not JSON ({error}): {line}"))
}

#[tokio::test]
#[ignore = "binds a socket and needs the official mem0ai Python SDK; run with --ignored"]
async fn official_mem0_sdk_drives_the_platform_wire() {
    let _env = lock_integration_test_env().await;
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
    println!("----- mem0ai driver ({python}) against {base_url} -----\n{stderr}");

    let result = parse_result(&stdout);
    let observed = &result["observed"];

    assert_eq!(
        result["ok"], true,
        "the driver reported failures: {}\n{}",
        result["failures"], stderr
    );
    assert_eq!(result["failures"], serde_json::json!([]), "{stderr}");

    // Provenance: a run against a vendored or stale copy must not be mistakable
    // for a run against the published package.
    let sdk = &result["sdk"];
    assert_eq!(sdk["package"], "mem0ai", "unexpected SDK package: {sdk}");
    let version = sdk["version"].as_str().unwrap_or_default();
    assert!(
        !version.is_empty(),
        "the driver could not establish the SDK version: {sdk}"
    );
    let module = sdk["module"].as_str().unwrap_or_default();
    assert!(
        !module.contains("external") && !module.contains("mem0-ts"),
        "the driver imported an in-repo copy, not the installed package: {module}"
    );

    // 1. `MemoryClient.__init__` already required a 2xx from `GET /v1/ping/`.
    assert_eq!(observed["ping"]["org_id"], EXPECTED_TENANT, "{observed}");
    assert_eq!(observed["ping"]["project_id"], EXPECTED_TENANT, "{observed}");

    // 2. `POST /v3/memories/add/`, with `user_id` at the **top level** of the
    //    body — the exact shape the framework's context-selector guard rejects
    //    unless the external protocol prefix is declared.
    let memory_id = observed["add"]["id"].as_str().expect("add returned an id");
    assert!(!memory_id.is_empty(), "{observed}");
    assert_eq!(observed["add"]["event"], "ADD", "{observed}");
    assert_eq!(observed["add"]["memory"], CONVERSATION, "{observed}");
    assert_eq!(observed["add"]["user_id"], "alice", "{observed}");
    assert_eq!(observed["add"]["agent_id"], "planner", "{observed}");
    assert_eq!(observed["add"]["metadata"]["topic"], "formatting", "{observed}");
    let hash = observed["add"]["hash"].as_str().unwrap_or_default();
    assert_eq!(hash.len(), 64, "`hash` must be a hex digest: {observed}");

    // 3. `GET /v1/memories/{memory_id}/`
    assert_eq!(observed["get"]["id"], memory_id, "{observed}");
    assert_eq!(observed["get"]["memory"], CONVERSATION, "{observed}");

    // 4. `POST /v3/memories/search/` with `filters` nested — which the guard must
    //    *not* object to, because it only inspects top-level keys.
    let ids = observed["search"]["ids"]
        .as_array()
        .expect("search reports ids");
    assert!(
        ids.iter().any(|id| id.as_str() == Some(memory_id)),
        "the memory must be retrievable by its own text: {observed}"
    );

    // 5. `PUT /v1/memories/{memory_id}/`
    assert_eq!(observed["update"]["id"], memory_id, "{observed}");
    assert_eq!(observed["update"]["memory"], UPDATED_TEXT, "{observed}");

    // 6. `GET /v1/memories/{memory_id}/history/` — a bare JSON array, which is
    //    what `MemoryClient.history` is typed to return.
    let events = observed["history"]["events"]
        .as_array()
        .expect("history reports events");
    let events: Vec<&str> = events.iter().filter_map(Value::as_str).collect();
    assert!(events.contains(&"ADD"), "history: {events:?}");
    assert!(events.contains(&"UPDATE"), "history: {events:?}");
    assert!(
        events
            .iter()
            .all(|event| ["ADD", "UPDATE", "DELETE"].contains(event)),
        "mem0 declares `event` a closed enum: {events:?}"
    );

    // 7. `GET /v1/entities/`
    let scopes = observed["users"]["scopes"]
        .as_array()
        .expect("users reports scopes");
    assert!(
        scopes.iter().any(|scope| {
            scope["type"] == "user" && scope["name"] == "alice" && scope["owner"] == EXPECTED_TENANT
        }),
        "the addressed user scope must be listed: {scopes:?}"
    );

    // 8. `DELETE /v1/memories/{memory_id}/`, then the read that proves it took.
    assert_eq!(
        observed["delete"]["message"], "Memory deleted successfully",
        "{observed}"
    );
    let after_delete = &observed["get_after_delete"]["error"];
    assert_eq!(
        after_delete["type"], "MemoryNotFoundError",
        "the client maps our 404 onto its own type: {after_delete}"
    );
    assert_eq!(after_delete["error_code"], "HTTP_404", "{after_delete}");
    // This surface's own wording must survive the framework's response
    // interceptor; the framework's status-derived "Not found" would mean the
    // media-type bridge has regressed.
    assert_eq!(
        after_delete["message"], "memory not found",
        "the failure must reach the client in this surface's own words: {after_delete}"
    );

    // 9. Every refusal must arrive as a readable, typed failure — never as a 2xx.
    let refusals = observed["refusals"]
        .as_array()
        .expect("refusals are recorded");
    assert_eq!(refusals.len(), 5, "all five refusals must be exercised: {refusals:?}");
    for refusal in refusals {
        let call = refusal["call"].as_str().unwrap_or("?");
        assert!(
            refusal.get("accepted").is_none(),
            "`{call}` was answered instead of refused: {refusal}"
        );
        let error = &refusal["error"];
        assert_eq!(error["error_code"], "HTTP_501", "`{call}`: {error}");
        let message = error["message"].as_str().unwrap_or_default();
        assert!(
            message.len() > 20,
            "`{call}` must explain itself, not just fail: {error}"
        );
    }

    // 10. The bulk sweep really empties the space.
    let sweep = observed["delete_all"]["message"]
        .as_str()
        .expect("delete_all is acknowledged");
    assert!(
        sweep.ends_with("memories deleted successfully"),
        "unexpected sweep acknowledgement: {sweep}"
    );
    assert_eq!(
        observed["list_after_sweep"]["count"], 0,
        "the space must be empty after the sweep: {observed}"
    );

    // --- the operations added for the second compatibility pass ---------------
    // Every value below is re-derived from the driver's raw observation rather
    // than read from its verdict, and the expected values are owned here.

    // 11. `POST /v1/feedback/`. The canonical record accepts a comment but does
    //     not read it back, so `feedback_reason` must come home null. Echoing the
    //     request would claim a round trip this surface cannot prove.
    let feedback_id = observed["feedback"]["id"].as_str().unwrap_or_default();
    assert!(!feedback_id.is_empty(), "{observed}");
    assert_eq!(observed["feedback"]["feedback"], "POSITIVE", "{observed}");
    assert!(
        observed["feedback"]["feedback_reason"].is_null(),
        "feedback_reason is not persisted or read back, so reporting a value \
         would be an unprovable echo: {observed}"
    );

    // 12. A withdrawal is refused, not silently answered. Upstream reads an
    //     omitted `feedback` as *clearing* the feedback; the canonical record is
    //     only ever written, never cleared, so a 2xx would report a mutation that
    //     did not happen.
    let withdrawal = &observed["feedback_withdrawal"];
    assert!(
        withdrawal.get("accepted").is_none(),
        "a withdrawal must not be answered: {withdrawal}"
    );
    assert_eq!(withdrawal["error"]["error_code"], "HTTP_501", "{withdrawal}");
    // 501 is outside the client's status table, so it lands on its base class —
    // still a readable, catchable failure rather than a raw transport error.
    assert_eq!(withdrawal["error"]["type"], "MemoryError", "{withdrawal}");
    assert!(
        withdrawal["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("feedback withdrawal")),
        "the refusal must name the operation: {withdrawal}"
    );

    // 13. The client guards its own feedback enum, so that request never opened a
    //     socket. Recorded to keep the two apart: the server's closed-enum check
    //     is pinned by `mem0_wire_flow.rs`, which does speak to the wire, and is
    //     deliberately *not* claimed as covered by this run.
    assert_eq!(observed["feedback_local_guard"]["type"], "ValueError", "{observed}");
    assert!(
        observed["feedback_local_guard"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("POSITIVE")),
        "the local guard must state the accepted values: {observed}"
    );

    // 14. `PUT /v1/batch/`. Upstream's 200 carries only a count in prose and no
    //     per-item channel, so the count is the *only* thing the response can say
    //     — the writes themselves have to be proved by reading both records back.
    assert_eq!(
        observed["batch_update"]["message"], "Successfully updated 2 memories",
        "{observed}"
    );
    assert_eq!(observed["batch_update"]["first"], BATCH_FIRST_UPDATED, "{observed}");
    assert_eq!(observed["batch_update"]["second"], BATCH_SECOND_UPDATED, "{observed}");
    assert_eq!(
        observed["batch_update"]["second_metadata"], "second",
        "a metadata patch must reach the record: {observed}"
    );

    // 15. Each boundary must be refused as a whole, for the stated reason. The
    //     status is not incidental: 404 is what the client turns into
    //     `MemoryNotFoundError`, and 400 into `ValidationError`.
    let rejections = observed["rejections"]
        .as_array()
        .expect("rejections are recorded");
    let expected: [(&str, &str, &str); 3] = [
        ("batch_update(unknown memory_id)", "HTTP_404", "MemoryNotFoundError"),
        (
            "batch_update(entry with no text or metadata)",
            "HTTP_400",
            "ValidationError",
        ),
        ("batch_update(1001 entries)", "HTTP_400", "ValidationError"),
    ];
    assert_eq!(rejections.len(), expected.len(), "{rejections:?}");
    for (rejection, (call, code, kind)) in rejections.iter().zip(expected) {
        assert_eq!(rejection["call"], call, "{rejections:?}");
        assert!(
            rejection.get("accepted").is_none(),
            "`{call}` was answered instead of refused: {rejection}"
        );
        let error = &rejection["error"];
        assert_eq!(error["error_code"], code, "`{call}`: {error}");
        assert_eq!(error["type"], kind, "`{call}`: {error}");
    }
    // The pre-flight is only worth anything if it stopped the whole batch: the
    // resolvable entry beside the unknown id must still hold its previous text.
    assert_eq!(
        rejections[0]["error"]["message"],
        format!("memory {UNKNOWN_ID} not found"),
        "the 404 must name the entry it could not resolve, and must arrive in \
         this surface's own words: {rejections:?}"
    );

    // 16. `DELETE /v1/batch/`, then the reads that prove both removals took.
    assert_eq!(
        observed["batch_delete"]["message"], "Successfully deleted 2 memories",
        "{observed}"
    );
    let proof = observed["batch_delete_proof"]
        .as_array()
        .expect("the post-delete reads are recorded");
    assert_eq!(proof.len(), 2, "{proof:?}");
    for entry in proof {
        assert!(entry.get("accepted").is_none(), "the record must be gone: {entry}");
        assert_eq!(entry["error"]["error_code"], "HTTP_404", "{entry}");
        assert_eq!(entry["error"]["type"], "MemoryNotFoundError", "{entry}");
        assert_eq!(entry["error"]["message"], "memory not found", "{entry}");
    }

    assert_eq!(exit_code, Some(0), "the driver exited {exit_code:?}\n{stderr}");

    println!(
        "official mem0ai {version} drove the platform wire end to end ({} refusal(s), {} rejection(s) classified)\n",
        refusals.len(),
        rejections.len(),
    );
}
