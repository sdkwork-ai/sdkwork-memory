//! The official **JavaScript** `mem0ai` client driving the mem0 platform wire.
//!
//! The second of the two official clients, and not a formality: it is a separate
//! codebase with its own request shaping, its own option surface and its own
//! error taxonomy, and reading it against this surface already found a real
//! incompatibility the Python run could not see (`status: "ok"` on
//! `GET /v1/ping/`, which this client requires and the Python one ignores).
//!
//! It is not a soft gate — run it explicitly and it fails loudly:
//!
//! ```text
//! MEM0_E2E_NODE=<node with mem0ai> \
//!   cargo test -p sdkwork-routes-memory-open-api \
//!     --test mem0_official_js_sdk_flow -- --ignored --nocapture
//! ```
//!
//! Discovery: `MEM0_E2E_NODE` (and `MEM0_E2E_SDK_ROOT`) if set, then `node` from
//! `PATH`, then the `workspace` sibling of the node installation this project's
//! runtime-isolation rules use. Each candidate is probed by *resolving* the
//! package, never by assuming a layout — the same lesson as the Python driver's
//! interpreter discovery: a gate that cannot find its runtime is a gate that
//! quietly stopped being re-runnable.
//!
//! Assertions live in `tests/mem0_official_sdk_support/mod.rs` and are shared
//! with the Python client's test. Where the clients differ, the difference is
//! declared and asserted in this client's own test rather than smoothed over.

#[path = "mem0_official_sdk_support/mod.rs"]
mod support;

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use support::{
    assert_wire_observations, build_app, mem0_api_key, parse_result, ClientExpectations,
    BATCH_FIRST, BATCH_FIRST_UPDATED, BATCH_SECOND, BATCH_SECOND_UPDATED, CONVERSATION,
    EXPECTED_TENANT, UNKNOWN_ID, UPDATED_TEXT,
};

/// The refusals this client attempts, in call order. Same set as the Python
/// client: this one reaches all five, even the two its TypeScript types do not
/// name for these calls, because it serialises whatever it is given.
const REFUSALS: [&str; 5] = [
    "get_all(filters=user_id)",
    "get_all(page=2)",
    "delete_all(user_id=...)",
    "update(timestamp=...)",
    "delete(delete_linked=True)",
];

/// Declared rather than counted, so a boundary that stops being probed is a
/// failure rather than a quietly smaller number.
const REJECTIONS: [(&str, &str, &str); 3] = [
    ("batch_update(unknown memory_id)", "HTTP_404", "MemoryNotFoundError"),
    (
        "batch_update(entry with no text or metadata)",
        "HTTP_400",
        "ValidationError",
    ),
    ("batch_update(1001 entries)", "HTTP_400", "ValidationError"),
];

/// Resolves the installed package to the entry this driver should import.
///
/// Prints the ESM entry when the package ships one, because that is what an
/// `import()` should load; otherwise the CJS entry, which Node can also import.
/// Resolving rather than guessing means a differently-laid-out installation
/// still works and a *missing* one fails loudly instead of being silently
/// skipped.
const SDK_PROBE: &str = r#"
const p = require("path"), fs = require("fs");
const resolved = require.resolve("mem0ai");
const root = p.resolve(p.dirname(resolved), "..");
const mjs = p.join(root, "dist", "index.mjs");
process.stdout.write(fs.existsSync(mjs) ? mjs : resolved);
"#;

/// Where the SDK may be installed, in probe order.
fn sdk_roots(node: &Path, repo_root: &Path) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Ok(explicit) = std::env::var("MEM0_E2E_SDK_ROOT") {
        if !explicit.trim().is_empty() {
            roots.push(PathBuf::from(explicit));
        }
    }
    // This project's runtime-isolation layout puts the isolated environment
    // beside the runtime: `<...>/node/versions/<ver>/node.exe` next to
    // `<...>/node/workspace`. Derive it from the executable rather than from a
    // hard-coded home directory, so the same code works for any installation
    // that follows the layout and simply finds nothing otherwise.
    if let Some(version_dir) = node.parent() {
        if let Some(versions_dir) = version_dir.parent() {
            if let Some(node_dir) = versions_dir.parent() {
                roots.push(node_dir.join("workspace"));
            }
        }
    }
    roots.push(repo_root.to_path_buf());
    roots
}

/// The absolute path behind a node *command*.
///
/// A bare `node` carries no layout information — `PathBuf::from("node")` has no
/// useful parent — so the installation cannot be located from the command name
/// alone. Asking the runtime where it actually lives is the only way to find the
/// isolated environment this project's rules put beside it.
fn resolve_node_path(node: &Path) -> Option<PathBuf> {
    let output = Command::new(node)
        .args(["-e", "process.stdout.write(process.execPath)"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let resolved = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if resolved.is_empty() {
        None
    } else {
        Some(PathBuf::from(resolved))
    }
}

/// A `(node, sdk entry)` pair that really can load the official client, or a
/// reason there is none.
fn find_node_with_mem0() -> Result<(PathBuf, PathBuf), String> {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate directory is always two levels below the repository root")
        .to_path_buf();

    let mut nodes: Vec<PathBuf> = Vec::new();
    if let Ok(explicit) = std::env::var("MEM0_E2E_NODE") {
        if !explicit.trim().is_empty() {
            nodes.push(PathBuf::from(explicit));
        }
    }
    nodes.push(PathBuf::from("node"));

    let mut tried = Vec::new();
    for command in nodes {
        // Resolve first: every candidate root below is derived from where the
        // runtime really is, not from how it was invoked.
        let Some(node) = resolve_node_path(&command) else {
            tried.push(format!("{} (not runnable)", command.display()));
            continue;
        };
        for root in sdk_roots(&node, &repo_root) {
            if !root.is_dir() {
                tried.push(format!(
                    "{} under {} (no such directory)",
                    node.display(),
                    root.display()
                ));
                continue;
            }
            let probe = Command::new(&node)
                .args(["-e", SDK_PROBE])
                .current_dir(&root)
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .output();
            match probe {
                Ok(output) if output.status.success() => {
                    let entry = String::from_utf8_lossy(&output.stdout).trim().to_owned();
                    if entry.is_empty() {
                        tried.push(format!(
                            "{} under {} (resolved nothing)",
                            node.display(),
                            root.display()
                        ));
                        continue;
                    }
                    return Ok((node, PathBuf::from(entry)));
                }
                Ok(output) => tried.push(format!(
                    "{} under {} (exit {})",
                    node.display(),
                    root.display(),
                    output.status
                )),
                Err(error) => tried.push(format!(
                    "{} under {} ({error})",
                    node.display(),
                    root.display()
                )),
            }
        }
    }

    Err(format!(
        "no node could resolve `mem0ai`; tried {}. Install it with \
         `npm install mem0ai` where node can see it, or point MEM0_E2E_SDK_ROOT at that directory \
         (and MEM0_E2E_NODE at the interpreter).",
        tried.join(", ")
    ))
}

/// Runs the driver and returns `(stdout, stderr, exit code)`.
fn run_driver(node: &Path, sdk_entry: &Path, base_url: &str) -> (String, String, Option<i32>) {
    let script = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/mem0_official_sdk/acceptance.mjs"
    );
    let output = Command::new(node)
        .arg(script)
        .env("MEM0_BASE_URL", base_url)
        .env("MEM0_API_KEY", mem0_api_key())
        .env("MEM0_EXPECT_TENANT", EXPECTED_TENANT)
        .env("MEM0_E2E_CONVERSATION", CONVERSATION)
        .env("MEM0_E2E_UPDATED_TEXT", UPDATED_TEXT)
        .env("MEM0_E2E_BATCH_FIRST", BATCH_FIRST)
        .env("MEM0_E2E_BATCH_SECOND", BATCH_SECOND)
        .env("MEM0_E2E_BATCH_FIRST_UPDATED", BATCH_FIRST_UPDATED)
        .env("MEM0_E2E_BATCH_SECOND_UPDATED", BATCH_SECOND_UPDATED)
        .env("MEM0_E2E_UNKNOWN_ID", UNKNOWN_ID)
        // The SDK is resolved by the harness and handed over by path, so the
        // driver never depends on the module resolver agreeing with the cwd.
        .env("MEM0_E2E_SDK_ENTRY", sdk_entry)
        // Lowercase deliberately: this SDK tests `=== "false"` where the Python
        // one accepts `"False"`. A hermetic acceptance must not phone home.
        .env("MEM0_TELEMETRY", "false")
        .output()
        .unwrap_or_else(|error| panic!("could not run `{} {script}`: {error}", node.display()));

    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
        output.status.code(),
    )
}

#[tokio::test]
#[ignore = "binds a socket and needs the official mem0ai JavaScript SDK; run with --ignored"]
async fn official_mem0_js_sdk_drives_the_platform_wire() {
    let _env = sdkwork_memory_test_support::web_auth::lock_integration_test_env().await;
    let app = build_app().await;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("an ephemeral loopback port binds");
    let address = listener.local_addr().expect("the bound address is known");
    let base_url = format!("http://{address}");

    let server = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    let (node, sdk_entry) = find_node_with_mem0().expect("a node that can load the official mem0ai");

    let served_url = base_url.clone();
    let joined = tokio::task::spawn_blocking(move || run_driver(&node, &sdk_entry, &served_url))
        .await
        .expect("the driver task does not panic");

    server.abort();

    let (stdout, stderr, exit_code) = joined;
    println!("----- mem0ai JavaScript driver against {base_url} -----\n{stderr}");

    let result = parse_result(&stdout);

    let version = assert_wire_observations(
        &result,
        &stderr,
        "mem0ai JavaScript",
        &ClientExpectations {
            refusals: &REFUSALS,
            // Five from the filter/paging list plus the feedback withdrawal.
            refusals_total: 6,
            rejections: &REJECTIONS,
            // The three batch payloads, plus the feedback-enum rejection this
            // client alone can deliver (see below), plus the two post-delete
            // read-backs.
            rejections_total: 6,
        },
    );

    // Client-specific, and the exact opposite of the Python client: this one has
    // **no** client-side feedback-enum guard, so an invalid value reaches the
    // wire and the *server's* closed-enum check has to answer. That is why this
    // run covers behaviour the Python run explicitly cannot claim, and why the
    // server-side check is asserted here rather than merely in a hand-built
    // request test.
    let enum_rejection = &result["observed"]["feedback_enum_rejection"];
    assert!(
        enum_rejection.get("accepted").is_none(),
        "the server must reject an enum value outside its own closed set: {enum_rejection}"
    );
    assert_eq!(
        enum_rejection["error"]["error_code"], "HTTP_400",
        "{enum_rejection}"
    );
    assert_eq!(enum_rejection["error"]["type"], "ValidationError", "{enum_rejection}");
    assert!(
        enum_rejection["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("POSITIVE")),
        "the rejection must state the accepted values: {enum_rejection}"
    );

    // Client-specific capability limit, recorded so it cannot be mistaken for
    // coverage: `batchUpdate` maps each entry to `{memory_id, text}` and drops
    // every other field, so this client cannot patch `metadata` through
    // `PUT /v1/batch/` at all. The Python test asserts the patch; here the
    // absence is the documented fact.
    assert!(
        result["observed"]["batch_update"].get("second_metadata").is_none(),
        "this client is not expected to be able to send a metadata patch: {}",
        result["observed"]["batch_update"]
    );

    assert_eq!(exit_code, Some(0), "the driver exited {exit_code:?}\n{stderr}");

    println!(
        "official mem0ai {version} (JavaScript) drove the platform wire end to end \
         ({} refusal(s), {} rejection(s) classified)\n",
        result["refusals_total"].as_u64().unwrap_or_default(),
        result["rejections_total"].as_u64().unwrap_or_default(),
    );
}
