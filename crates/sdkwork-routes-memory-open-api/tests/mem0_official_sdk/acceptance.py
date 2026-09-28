"""Drive the **official** `mem0ai` client against a running Memory service.

This is the acceptance half of the mem0 platform compatibility claim: nothing
here talks to the service by hand. `MemoryClient` builds every request, decides
which endpoint to call, parses every response, and raises its own exception
classes on failure. If the wire only matched in our own test harness but not
this client, the run fails.

Provenance is reported, not assumed: the package name, version, and module path
are read from the interpreter actually used and echoed into the result, so a run
against a stale or vendored copy cannot be mistaken for a run against the
published SDK.

Everything this surface *refuses* is exercised too, and classified. A capability
gap that the client can read and act on is a compatibility decision; the same
gap silently degrading into a plausible-looking answer would be a defect. The
refusals are therefore asserted to arrive as the client's own typed exceptions
carrying this surface's own reason string.

A rejection is distinguished from a *local* guard: the client validates some
inputs before it ever opens a socket (`feedback()` raises `ValueError` on a
value outside its own enum). Such a case is recorded separately, because passing
it off as wire coverage would claim a server-side check had been exercised when
the request was never sent.

Output: one JSON object on the final line, prefixed by `RESULT_PREFIX`. The Rust
test parses it (`tests/mem0_official_sdk_flow.rs`); the human-readable narrative
goes to stderr so it can never corrupt the machine-readable result.
"""

import json
import os
import sys
import traceback

# Set before `mem0` is imported: the package reads these at import time.
# Telemetry is off so the run cannot depend on, or leak to, a third party, and
# `MEM0_DIR` keeps the SDK's own config/anon-id files out of the user's home.
os.environ.setdefault("MEM0_TELEMETRY", "False")

RESULT_PREFIX = "MEM0_E2E_RESULT "

CONVERSATION = os.environ["MEM0_E2E_CONVERSATION"]
UPDATED_TEXT = os.environ["MEM0_E2E_UPDATED_TEXT"]
BASE_URL = os.environ["MEM0_BASE_URL"]
API_KEY = os.environ["MEM0_API_KEY"]
EXPECTED_TENANT = os.environ["MEM0_EXPECT_TENANT"]

# The user the memory is filed under. Distinct from the authenticated actor:
# mem0's entity scopes are a data dimension, not a credential.
USER_ID = "alice"

# Two subjects for the batch path, so a batch can be shown to address more than
# one record. The updated forms differ from the originals, which is what makes
# "the batch really wrote" observable after the fact. All of these are supplied
# by the caller rather than hardcoded, so the expected values live in one place.
BATCH_FIRST = os.environ["MEM0_E2E_BATCH_FIRST"]
BATCH_SECOND = os.environ["MEM0_E2E_BATCH_SECOND"]
BATCH_FIRST_UPDATED = os.environ["MEM0_E2E_BATCH_FIRST_UPDATED"]
BATCH_SECOND_UPDATED = os.environ["MEM0_E2E_BATCH_SECOND_UPDATED"]

# A `memory_id` that names nothing. A valid unsigned integer, so the surface
# rejects it for being absent rather than malformed — different code paths, and
# only the first is under test here.
UNKNOWN_ID = os.environ["MEM0_E2E_UNKNOWN_ID"]

failures = []


def note(message):
    print(message, file=sys.stderr, flush=True)


def check(name, condition, detail=""):
    if condition:
        note(f"  ok   {name}")
    else:
        note(f"  FAIL {name} {detail}")
        failures.append(f"{name} {detail}".strip())


def describe(exc):
    """The shape a caller sees: type, machine code, human message."""
    return {
        "type": type(exc).__name__,
        "error_code": getattr(exc, "error_code", None),
        "message": getattr(exc, "message", None) or str(exc),
    }


def expect_refusal(label, call):
    """Run something this surface refuses and record how the client saw it.

    A refusal must arrive as the client's own exception type carrying a non-empty
    reason. A 2xx here would be the failure mode that matters: the client told a
    mutation succeeded when it did not happen.
    """
    try:
        value = call()
    except Exception as exc:  # noqa: BLE001 - the exception *is* the observation
        observed = describe(exc)
        note(f"  refused {label}: {observed['type']} {observed['error_code']}")
        check(f"{label} carries a reason", bool(observed["message"]), observed)
        return {"call": label, "error": observed}
    note(f"  FAIL {label} was accepted: {value!r}")
    failures.append(f"{label} was accepted instead of refused: {value!r}")
    return {"call": label, "accepted": value}


def expect_error(label, call):
    """Run something with a wrong parameter and record how the client saw it.

    Distinct from a refusal: here the request is well-formed but names something
    the surface cannot do as asked, so the caller's own input is at fault. The
    point is the same either way — it must not be answered with a plausible
    result.
    """
    try:
        value = call()
    except Exception as exc:  # noqa: BLE001
        observed = describe(exc)
        note(f"  rejected {label}: {observed['type']} {observed['error_code']}")
        check(f"{label} carries a reason", bool(observed["message"]), observed)
        return {"call": label, "error": observed}
    note(f"  FAIL {label} was accepted: {value!r}")
    failures.append(f"{label} was accepted: {value!r}")
    return {"call": label, "accepted": value}


def main():
    from mem0.client.main import MemoryClient

    import mem0

    try:
        from importlib.metadata import version as package_version

        sdk_version = package_version("mem0ai")
    except Exception:  # noqa: BLE001 - provenance must not be able to fail the run
        sdk_version = None

    sdk = {
        "package": "mem0ai",
        "version": sdk_version,
        "module": mem0.__file__,
        "base_url": BASE_URL,
    }
    note(f"official SDK: mem0ai {sdk_version} from {sdk['module']}")

    observed = {}

    # The constructor is itself a wire test: it calls `GET /v1/ping/` and raises
    # if it is not a 2xx. Reaching the next line proves ping, the credential
    # bridge, and the media type all agree with the client.
    client = MemoryClient(api_key=API_KEY, host=BASE_URL)
    observed["ping"] = {
        "org_id": client.org_id,
        "project_id": client.project_id,
        "user_email": client.user_email,
    }
    note(f"ping: {observed['ping']}")
    check("ping reports the tenant as project", client.org_id == EXPECTED_TENANT, observed["ping"])
    check("ping reports the tenant as org", client.project_id == EXPECTED_TENANT, observed["ping"])

    # --- write -----------------------------------------------------------------
    added = client.add(
        [{"role": "user", "content": CONVERSATION}, {"role": "assistant", "content": ""}],
        user_id=USER_ID,
        agent_id="planner",
        metadata={"topic": "formatting"},
    )
    created = added["results"][0]
    memory_id = created["id"]
    observed["add"] = {
        "id": memory_id,
        "event": created.get("event"),
        "memory": created.get("memory"),
        "user_id": created.get("user_id"),
        "agent_id": created.get("agent_id"),
        "hash": created.get("hash"),
        "metadata": created.get("metadata"),
        "created_at": created.get("created_at"),
        "updated_at": created.get("updated_at"),
    }
    note(f"add -> {memory_id}")
    check("add returns an id", bool(memory_id))
    check("add reports event ADD", created.get("event") == "ADD", created.get("event"))
    check("add returns the memory text", created.get("memory") == CONVERSATION)
    check("add echoes the user scope", created.get("user_id") == USER_ID)
    check("add keeps caller metadata", (created.get("metadata") or {}).get("topic") == "formatting")

    # --- read back -------------------------------------------------------------
    fetched = client.get(memory_id)
    observed["get"] = {"id": fetched.get("id"), "memory": fetched.get("memory")}
    check("get returns the same id", fetched.get("id") == memory_id)

    found = client.search("bullet-point summaries", filters={"user_id": USER_ID}, top_k=5)
    hits = found["results"]
    observed["search"] = {
        "count": len(hits),
        "ids": [hit.get("id") for hit in hits],
        "first": hits[0] if hits else None,
    }
    note(f"search -> {len(hits)} hit(s)")
    check("search finds the stored memory", memory_id in observed["search"]["ids"], observed["search"]["ids"])
    check("search honours the metadata filter", len(hits) >= 1)

    updated = client.update(memory_id, text=UPDATED_TEXT)
    observed["update"] = {"id": updated.get("id"), "memory": updated.get("memory")}
    check("update rewrites the text", updated.get("memory") == UPDATED_TEXT, updated.get("memory"))

    # --- feedback --------------------------------------------------------------
    # The canonical feedback record accepts a comment but does not read it back,
    # so `feedback_reason` is expected to come home as null rather than echoed.
    feedback = client.feedback(
        memory_id,
        feedback="POSITIVE",
        feedback_reason="matched what the operator asked for",
    )
    observed["feedback"] = {
        "id": feedback.get("id"),
        "feedback": feedback.get("feedback"),
        "feedback_reason": feedback.get("feedback_reason"),
    }
    note(f"feedback -> {observed['feedback']}")
    check("feedback reports an id", bool(feedback.get("id")), observed["feedback"])
    check("feedback reports the stored value", feedback.get("feedback") == "POSITIVE", observed["feedback"])

    # A withdrawal. Upstream models `feedback: null` as clearing the feedback, but
    # the canonical feedback record is only ever written and never cleared, so
    # there is no value to answer with. A 2xx here would report a mutation that
    # did not happen.
    withdrawal = expect_refusal("feedback(no value, i.e. withdrawal)", lambda: client.feedback(memory_id))
    observed["feedback_withdrawal"] = withdrawal
    # `feedback()` guards its own enum client-side, so a bad value never reaches
    # the wire. Recorded as a *local* guard rather than a rejection: the server's
    # own closed-enum check is exercised by `mem0_wire_flow.rs`, which does speak
    # to the wire. Claiming this line as server coverage would be false.
    try:
        client.feedback(memory_id, feedback="MAYBE")
        observed["feedback_local_guard"] = {"accepted": True}
        failures.append("the client forwarded a value outside its own feedback enum")
    except ValueError as exc:
        observed["feedback_local_guard"] = {"type": "ValueError", "message": str(exc)}
        note("  local guard feedback(feedback='MAYBE'): ValueError")

    history = client.history(memory_id)
    events = [entry.get("event") for entry in history]
    observed["history"] = {"events": events, "entries": history}
    note(f"history -> {events}")
    check("history records the add", "ADD" in events, events)
    check("history records the update", "UPDATE" in events, events)
    check(
        "history stays inside mem0's closed event enum",
        all(event in ("ADD", "UPDATE", "DELETE") for event in events),
        events,
    )

    entities = client.users()
    scopes = entities["results"]
    observed["users"] = {
        "count": entities.get("count"),
        "scopes": [{"type": s.get("type"), "name": s.get("name"), "owner": s.get("owner")} for s in scopes],
    }
    note(f"users -> {[s['type'] + ':' + str(s['name']) for s in observed['users']['scopes']]}")
    check(
        "users lists the addressed user scope",
        any(s["type"] == "user" and s["name"] == USER_ID and s["owner"] == EXPECTED_TENANT for s in observed["users"]["scopes"]),
        observed["users"]["scopes"],
    )

    # --- delete, and prove it took ---------------------------------------------
    deleted = client.delete(memory_id)
    observed["delete"] = {"message": deleted.get("message")}
    check("delete is acknowledged", bool(deleted.get("message")), deleted)

    try:
        client.get(memory_id)
        observed["get_after_delete"] = {"accepted": True}
        failures.append("a deleted memory was still readable")
        note("  FAIL deleted memory still readable")
    except Exception as exc:  # noqa: BLE001
        observed["get_after_delete"] = {"error": describe(exc)}
        note(f"  deleted memory reads back as {type(exc).__name__} {getattr(exc, 'error_code', None)}")
        check("deleted memory raises MemoryNotFoundError", type(exc).__name__ == "MemoryNotFoundError", observed["get_after_delete"])

    # --- batch: `PUT` and `DELETE` share one path -------------------------------
    # Upstream acknowledges a batch with a count in prose and *no* per-item result
    # channel, so a batch that half-applied could not be reported as anything but a
    # success. Both directions are driven here, and the boundaries that would
    # otherwise be discovered mid-write are driven too — each must be refused as a
    # whole, leaving the entries it could resolve untouched.
    note("batch:")
    batch_first = client.add([{"role": "user", "content": BATCH_FIRST}], user_id=USER_ID)
    batch_second = client.add([{"role": "user", "content": BATCH_SECOND}], user_id=USER_ID)
    batch_first_id = batch_first["results"][0]["id"]
    batch_second_id = batch_second["results"][0]["id"]

    acked = client.batch_update(
        [
            {"memory_id": batch_first_id, "text": BATCH_FIRST_UPDATED},
            {"memory_id": batch_second_id, "text": BATCH_SECOND_UPDATED, "metadata": {"batch": "second"}},
        ]
    )
    read_first = client.get(batch_first_id)
    read_second = client.get(batch_second_id)
    observed["batch_update"] = {
        "message": acked.get("message"),
        "first": read_first.get("memory"),
        "second": read_second.get("memory"),
        "second_metadata": (read_second.get("metadata") or {}).get("batch"),
    }
    note(f"batch_update -> {observed['batch_update']['message']}")
    check(
        "batch_update counts both entries",
        acked.get("message") == "Successfully updated 2 memories",
        observed["batch_update"],
    )
    check("batch_update rewrote the first text", read_first.get("memory") == BATCH_FIRST_UPDATED, observed["batch_update"])
    check("batch_update rewrote the second text", read_second.get("memory") == BATCH_SECOND_UPDATED, observed["batch_update"])
    check(
        "batch_update carried the metadata patch",
        observed["batch_update"]["second_metadata"] == "second",
        observed["batch_update"],
    )

    observed["rejections"] = [
        # An id that names nothing. Because the acknowledgement has no per-item
        # channel, this must abort before the resolvable entry beside it is written.
        expect_error(
            "batch_update(unknown memory_id)",
            lambda: client.batch_update(
                [
                    {"memory_id": batch_first_id, "text": "must not be written"},
                    {"memory_id": UNKNOWN_ID, "text": "names nothing"},
                ]
            ),
        ),
        # An update entry carrying neither payload. The canonical update refuses an
        # empty patch, and that must be found before any sibling is written.
        expect_error(
            "batch_update(entry with no text or metadata)",
            lambda: client.batch_update([{"memory_id": batch_first_id}]),
        ),
        # Upstream declares `maxItems: 1000` on the request array itself.
        expect_error(
            "batch_update(1001 entries)",
            lambda: client.batch_update([{"memory_id": batch_first_id, "text": "overflow"} for _ in range(1001)]),
        ),
    ]
    check(
        "an aborted batch wrote nothing",
        client.get(batch_first_id).get("memory") == BATCH_FIRST_UPDATED,
        observed["batch_update"],
    )

    removed = client.batch_delete([{"memory_id": batch_first_id}, {"memory_id": batch_second_id}])
    observed["batch_delete"] = {"message": removed.get("message")}
    note(f"batch_delete -> {observed['batch_delete']['message']}")
    check(
        "batch_delete counts both entries",
        removed.get("message") == "Successfully deleted 2 memories",
        observed["batch_delete"],
    )
    observed["batch_delete_proof"] = [
        expect_error("get(first, deleted by batch)", lambda: client.get(batch_first_id)),
        expect_error("get(second, deleted by batch)", lambda: client.get(batch_second_id)),
    ]

    # --- refusals: parameters and filters this surface cannot honour ------------
    # These are the calls an ordinary mem0 caller makes. They are refused because
    # answering them would mean inventing a result; each must say so, by name.
    note("refusals:")
    # A second, unrelated memory so the two per-memory refusals below have a
    # target that is not the one already deleted above.
    sibling = client.add([{"role": "user", "content": CONVERSATION}], user_id="bob")
    sibling_id = sibling["results"][0]["id"]
    observed["refusals"] = [
        expect_refusal("get_all(filters=user_id)", lambda: client.get_all(filters={"user_id": USER_ID})),
        expect_refusal("get_all(page=2)", lambda: client.get_all(page=2)),
        expect_refusal("delete_all(user_id=...)", lambda: client.delete_all(user_id=USER_ID)),
        expect_refusal(
            "update(timestamp=...)",
            lambda: client.update(sibling_id, timestamp="2026-01-01T00:00:00Z"),
        ),
        expect_refusal("delete(delete_linked=True)", lambda: client.delete(sibling_id, delete_linked=True)),
    ]

    # --- bulk sweep ------------------------------------------------------------
    swept = client.delete_all()
    observed["delete_all"] = {"message": swept.get("message")}
    check("delete_all is acknowledged", bool(swept.get("message")), swept)

    empty = client.get_all()
    observed["list_after_sweep"] = {
        "count": empty.get("count"),
        "results": len(empty.get("results") or []),
        "next": empty.get("next"),
        "previous": empty.get("previous"),
    }
    check("the sweep really emptied the space", empty.get("count") == 0, observed["list_after_sweep"])
    check("an unfiltered listing is answered", observed["list_after_sweep"]["results"] == 0)

    result = {"sdk": sdk, "observed": observed, "failures": failures, "ok": not failures}
    print(RESULT_PREFIX + json.dumps(result), flush=True)
    return 0 if not failures else 1


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception:  # noqa: BLE001
        traceback.print_exc(file=sys.stderr)
        print(
            RESULT_PREFIX
            + json.dumps({"sdk": None, "observed": {}, "failures": ["the driver itself crashed"], "ok": False}),
            flush=True,
        )
        sys.exit(2)
