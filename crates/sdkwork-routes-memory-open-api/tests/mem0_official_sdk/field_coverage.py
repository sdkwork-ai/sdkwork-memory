"""Field-coverage probe: the official Python client with EVERY declared option field set.

Companion to `field_capture_stub.py`. The question is not "does it work" but
"which keys does this client actually put on the wire" — so every option the SDK
declares is populated with a sentinel, and the stub records the request verbatim.
Reading the client's source instead would be guesswork: `_prepare_payload` dumps
`**kwargs` straight into the body, so which keys appear depends on the call.

    python field_capture_stub.py 6321 /tmp/capture.jsonl &
    MEM0_BASE_URL=http://127.0.0.1:6321 MEM0_API_KEY=probe \
        python field_coverage.py

Interpreter: the one that has `mem0ai` installed, i.e. the same discovery contract
as `acceptance.py` (`MEM0_E2E_PYTHON`). Every call is emitted as one JSON line.
"""
import json
import os

from mem0 import MemoryClient

BASE = os.environ["MEM0_BASE_URL"]
KEY = os.environ["MEM0_API_KEY"]
TS = 1700000000


def emit(label, ok, **extra):
    print(json.dumps({"label": label, "ok": ok, **extra}, ensure_ascii=False), flush=True)


def attempt(label, fn):
    try:
        return fn()
    except Exception as error:  # noqa: BLE001 - the audit wants the taxonomy
        emit(label, False, exc=type(error).__name__, msg=str(error)[:400])
        return None


client = MemoryClient(api_key=KEY, host=BASE)
emit("client.init()", True)

added = attempt(
    "add(full surface)",
    lambda: client.add(
        messages=[{"role": "user", "content": "probe field coverage"}],
        user_id="probe-user",
        agent_id="probe-agent",
        app_id="probe-app",
        run_id="probe-run",
        filters={"user_id": "probe-user"},
        metadata={"probe": "add"},
        infer=True,
        custom_categories=[{"probe": "category"}],
        custom_instructions="probe custom instructions",
        agent_custom_instructions="probe agent instructions",
        timestamp=TS,
        expiration_date="2030-01-01",
        structured_data_schema={"type": "object", "properties": {}},
    ),
)

memory_id = None
if isinstance(added, dict):
    emit("add(full surface) -> body", True, keys=sorted(added.keys()))
    for item in added.get("results") or []:
        if isinstance(item, dict) and item.get("id"):
            memory_id = item["id"]
            break

listed = attempt(
    "get_all(full surface)",
    lambda: client.get_all(
        filters={"user_id": "probe-user"},
        page=2,
        page_size=5,
        start_date="2026-01-01",
        end_date="2026-12-31",
        categories=["probe-category"],
        show_expired=True,
        latest_only=True,
    ),
)
if isinstance(listed, dict):
    emit("get_all(full surface) -> body", True, keys=sorted(listed.keys()))
    if not memory_id:
        for item in listed.get("results") or []:
            if isinstance(item, dict) and item.get("id"):
                memory_id = item["id"]
                break

memory_id = memory_id or "00000000-0000-0000-0000-000000000000"
emit("resolved memory_id", True, memory_id=memory_id)

searched = attempt(
    "search(full surface)",
    lambda: client.search(
        "probe query",
        filters={"user_id": "probe-user"},
        metadata={"probe": "search"},
        top_k=7,
        rerank=True,
        threshold=0.5,
        fields=["memory", "id"],
        categories=["probe-category"],
        show_expired=True,
        reference_date="2026-01-01",
        latest_only=True,
        keyword_search=True,
    ),
)
if isinstance(searched, dict):
    emit("search(full surface) -> body", True, keys=sorted(searched.keys()))

updated = attempt(
    "update(full surface)",
    lambda: client.update(
        memory_id,
        text="probe updated text",
        metadata={"probe": "update"},
        timestamp=TS,
        expiration_date="2030-01-01",
    ),
)
if updated is not None:
    emit(
        "update(full surface) -> body",
        True,
        keys=sorted(updated.keys()) if isinstance(updated, dict) else None,
    )

for label, call in [
    ("get(memory_id)", lambda: client.get(memory_id)),
    ("history(memory_id)", lambda: client.history(memory_id)),
    ("users()", lambda: client.users()),
    ("feedback(POSITIVE)", lambda: client.feedback(memory_id, "POSITIVE", "probe reason")),
    (
        "batch_update(entry)",
        lambda: client.batch_update(
            [
                {
                    "memory_id": memory_id,
                    "text": "probe batch text",
                    "metadata": {"probe": "batch"},
                    "timestamp": TS,
                    "expiration_date": "2030-01-01",
                }
            ]
        ),
    ),
    (
        "batch_delete(entry)",
        lambda: client.batch_delete(
            [{"memory_id": memory_id, "text": "probe batch delete", "metadata": {"probe": "batch"}}]
        ),
    ),
    ("delete_all(filters=user_id)", lambda: client.delete_all(filters={"user_id": "probe-user"})),
    ("delete(delete_linked=True)", lambda: client.delete(memory_id, delete_linked=True)),
]:
    result = attempt(label, call)
    if result is not None:
        emit(
            label + " -> body",
            True,
            keys=sorted(result.keys()) if isinstance(result, dict) else None,
        )

emit("driver.done", True)
