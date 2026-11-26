"""Forward the Reader trial's compaction requests and retain actual draft/usage evidence."""
import argparse
import json
import os
from pathlib import Path
import runpy
import threading
from urllib.error import HTTPError
from urllib.parse import urlparse
from urllib.request import Request, urlopen


# At the verified Flash peak rate (2 / 8 CNY per million input / output),
# four 256 KiB requests, even at one token per byte, plus 8,192 tokens of
# output per request reserve less than CNY 2.5 including protocol overhead.
MAX_CALLS = 4
MAX_REQUEST_BYTES = 256 * 1024
MAX_OUTPUT_TOKENS = 8192
TIMEOUT_SECONDS = 150
MAX_COST_MICRO_CNY = 2_500_000


def save(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2), encoding="utf-8")


class LiveDrafts:
    """The caller supplies only requests identified by the runtime compaction contract."""

    def __init__(self, output_dir, prior_spend=0, max_sends=MAX_CALLS, output_limit=None):
        self.output_dir = Path(output_dir)
        self.output_dir.mkdir(parents=True, exist_ok=False)
        self.model = os.environ["FLUID_LLM_MODEL"]
        self.base = os.environ["OPENCODE_BASE_URL"].rstrip("/")
        self.key = os.environ["OPENCODE_API_KEY"]
        if (urlparse(self.base).scheme != "https" or urlparse(self.base).hostname != "api.deepseek.com"
                or self.model not in ("deepseek-flash", "deepseek-v4-flash")):
            raise ValueError("Live draft budget is calibrated for the official DeepSeek Flash endpoint")
        self.receipts = []
        self.send_lock = threading.Lock()
        self.prior_spend = prior_spend
        self.max_sends = max_sends
        self.output_limit = output_limit

    def __call__(self, body, ordinal=None):
        with self.send_lock:
            return self.forward(body, ordinal)

    def forward(self, body, ordinal):
        index = len(self.receipts) + 1
        if index > self.max_sends:
            raise RuntimeError("Live draft send limit reached")
        data = dict(body)
        data.update(model=self.model, stream=False,
                    max_tokens=min(self.output_limit or data.get("max_tokens") or MAX_OUTPUT_TOKENS, MAX_OUTPUT_TOKENS))
        data.pop("stream_options", None)
        payload = json.dumps(data, ensure_ascii=False, separators=(",", ":")).encode()
        if len(payload) > MAX_REQUEST_BYTES:
            raise RuntimeError(f"Live draft request has {len(payload)} bytes, limit {MAX_REQUEST_BYTES}")
        if "compaction" not in payload.decode().lower():
            raise ValueError("Missing compaction instruction in Reader request")

        # Peak uncached input and output, plus 4,096 input tokens for protocol
        # framing. Keep the reservation for any send without a usage receipt.
        reserve = (len(payload) + 4096) * 2 + data["max_tokens"] * 8
        committed = sum(r.get("peak_micro_cny", r["reserved_micro_cny"]) for r in self.receipts)
        if self.prior_spend + committed + reserve > MAX_COST_MICRO_CNY:
            raise RuntimeError("Live draft CNY budget reached before sending")
        prefix = self.output_dir / f"send-{index}"
        save(prefix.with_suffix(".request.json"), data)
        receipt = {"send": index, "ordinal": ordinal, "model": self.model, "wire_bytes": len(payload),
                   "max_tokens": data["max_tokens"], "state": "sent", "reserved_micro_cny": reserve}
        self.receipts.append(receipt)
        ledger = self.output_dir / "provider-sends.json"
        save(ledger, self.receipts)
        request = Request(self.base + "/chat/completions", data=payload, headers={
            "Authorization": "Bearer " + self.key, "Content-Type": "application/json"})
        try:
            with urlopen(request, timeout=TIMEOUT_SECONDS) as response:
                result = json.load(response)
                receipt.update(status=response.status, returned_model=result.get("model"), usage=result.get("usage"))
                usage = receipt.get("usage") or {}
                if "prompt_tokens" in usage and "completion_tokens" in usage:
                    cached = usage.get("prompt_cache_hit_tokens", 0)
                    receipt["peak_micro_cny"] = ((usage["prompt_tokens"] - cached) * 2
                                                + (cached + 24) // 25 + usage["completion_tokens"] * 8)
            choice = result["choices"][0]
            content = choice["message"]["content"]
            receipt.update(finish_reason=choice["finish_reason"], state="returned")
            # Preserve the verbatim candidate even when the runtime rejects it.
            prefix.with_suffix(".draft.txt").write_text(content or "", encoding="utf-8")
            if choice["message"].get("tool_calls"):
                raise ValueError("Provider called a tool during compaction")
            return content or ""
        except HTTPError as error:
            receipt.update(state="error", status=error.code,
                           provider_error=error.read(1500).decode("utf-8", errors="replace"))
            raise
        except (OSError, ValueError, KeyError, TypeError) as error:
            receipt.update(state="error", error=str(error))
            raise
        finally:
            save(ledger, self.receipts)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, required=True, help="New directory for Reader and paid sample evidence")
    parser.add_argument("--reader", type=Path, default=Path("target/debug/reader.exe"))
    parser.add_argument("--book", type=Path, default=Path(".understand-book/quickstart-demo"))
    parser.add_argument("--repeats", type=int, default=650)
    parser.add_argument("--retry-second", type=Path, help="Reuse the first accepted draft from the prior sample, then allow two sends at 8,192 output tokens")
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=False)
    output = args.out
    harness = runpy.run_path(str(Path(__file__).with_name("verify-long-conversation-compaction.py")))
    if args.retry_second:
        previous = args.retry_second / "provider"
        sends = json.loads((previous / "provider-sends.json").read_bytes())
        spent = sum(r.get("peak_micro_cny", r["reserved_micro_cny"]) for r in sends)
        upstream = LiveDrafts(output / "provider", prior_spend=spent, max_sends=2, output_limit=8192)
        sources = harness["compact_sources"](json.loads((previous / "send-1.request.json").read_bytes()))
        preserved = (previous / "send-1.draft.txt").read_text(encoding="utf-8")

        def generator(body, ordinal):
            if harness["compact_sources"](body) == sources:
                return preserved
            return upstream(body, ordinal)

    else:
        generator = LiveDrafts(output / "provider")
    args.out = args.out / "reader"
    args.bad_draft = False
    harness["run"](args, generator)
