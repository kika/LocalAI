# GLM-5.2 on 4× AMD MI300X — single-user, 1M-context coding agent

Boots [GLM-5.2](https://huggingface.co/zai-org/GLM-5.2) (4-bit) as an OpenAI-compatible
vLLM server tuned for a **single-user coding agent at up to 1M context** on a
**4× MI300X** box (ROCm).

## TL;DR

```bash
./boot_glm52.sh          # installs deps, downloads weights, picks a working image, serves
# → OpenAI API at http://<host>:8000/v1   (model name: glm-5.2)
```

Re-running is safe/idempotent: it skips the download and image pull if already done and
recreates the server container.

## Why this configuration

- **GLM-5.2** is a ~744B-parameter MoE (~40B active) using **MLA** (576-dim latent KV cache)
  and **DSA / IndexShare** sparse attention. Architecture id: `glm_moe_dsa`, **78 layers**.
- **4× MI300X = ~767 GiB HBM** (192 GiB/GPU). At BF16 (~1.5 TB) or FP8 (~750 GB) the weights
  don't leave room for KV cache on 4 GPUs → **use a 4-bit checkpoint**
  (`QuantTrio/GLM-5.2-Int4-Int8Mix`, ~378 GB). That leaves ~390 GB for KV/activations.
- Because GLM-5.2 uses **MLA**, the KV cache is tiny: a single **1M-token** sequence is only
  **~45 GB at fp8** (≈576 elems/token × 78 layers). Memory is a non-issue; the real cost at
  1M is **prefill latency**, which is why prefix caching matters most for an agent.

## What the script does

1. **Preflight** — checks GPUs/docker/disk, disables NUMA auto-balancing (MI300X best practice).
2. **HF CLI** — installs `huggingface_hub` in a venv (Ubuntu 24.04 is PEP668-managed; needs
   `python3-venv` via apt). Uses **Xet** high-performance transfer.
3. **Download** — `QuantTrio/GLM-5.2-Int4-Int8Mix` (~378 GB, resumable) to `~/models/...`.
4. **Image auto-select** — probes candidate images and picks the first whose vLLM supports
   `GlmMoeDsaForCausalLM` (see finding below).
5. **Serve** — launches the vLLM container with the tuned flags.
6. **Health + smoke test** — waits for `/v1/models`, runs a tiny completion.

## Key runtime flags (single-user, 1M) — validated on the target box

| Flag / env | Value | Why |
|---|---|---|
| `--tensor-parallel-size` | 4 | all 4 GPUs |
| `--enable-expert-parallel` | on | even MoE expert sharding |
| `--max-model-len` | 1048576 | full 1M context (MLA makes the KV affordable) |
| `--kv-cache-dtype` | fp8 | needed to fit 1M KV on 4 GPUs; verified coherent here |
| `--gpu-memory-utilization` | 0.90 | leaves physical headroom (see Memory tuning) |
| `--max-num-batched-tokens` | 2048 | shrinks prefill activation so 1M KV fits + headroom |
| `--enable-prefix-caching` | on | **biggest win** — reuses KV across agent turns instead of re-prefilling |
| `--max-num-seqs` | 4 | single user; keeps KV/compute for the long sequence |
| `--tool-call-parser` | `glm47` | GLM-5.2 tool-call format |
| `--reasoning-parser` | `glm45` | GLM-5.2 thinking parser |
| `--enforce-eager` | on | CUDA-graph capture OOMs at 1M; eager is the reliable default |
| `VLLM_ROCM_USE_AITER` | 1 | **required** — GLM-5.2 sparse attention has no non-AITER ROCm path |
| `VLLM_ROCM_USE_AITER_MOE` | **0** | **CRITICAL** — AITER MoE kernel emits GARBAGE for this W4A16 build on gfx942 |

Tune via env vars, e.g. `KV_CACHE_DTYPE=bf16 EAGER=0 PORT=9000 ./boot_glm52.sh`.

## Findings during implementation (important — these cost real debugging)

1. **Stable `rocm/vllm:latest` does NOT support GLM-5.2.** It ships vLLM 0.11.2 which only
   knows `Glm4MoeForCausalLM` (GLM-4.5/4.6), not `GlmMoeDsaForCausalLM`.
   **`rocm/vllm-dev:nightly`** (vLLM 0.23.1rc1) supports it. The script auto-detects via
   `IMAGE_CANDIDATES` (probes each image's `ModelRegistry`).
2. **The AITER fused-MoE kernel produces gibberish** for the QuantTrio W4A16 checkpoint on
   MI300X (gfx942). The server starts fine and generates tokens, but output is incoherent
   word-salad. Fix: `VLLM_ROCM_USE_AITER_MOE=0` (keep AITER on for the *required* sparse
   attention; route MoE through the non-AITER path). This was the single hardest bug here.
3. **AITER is mandatory.** `VLLM_ROCM_USE_AITER=0` fails fast: *"Sparse attention indexer
   ROCm path is only supported on AITER."* So you can't sidestep #2 by disabling AITER wholesale.
4. **Tool/reasoning parsers differ from GLM-4.6:** `--tool-call-parser glm47` (not `glm45`)
   and `--reasoning-parser glm45`, per the official vLLM recipe.
5. **MTP speculative decoding does not work with this checkpoint.** The MTP-block experts
   aren't group-quantized, so vLLM's WNA16 MoE method asserts and crashes at load. Disabled
   by default (`MTP=0`). Would need an FP8 build or a group-quant MTP checkpoint.
6. **Memory at 1M is a tightrope** — see next section.
7. **Download is Xet, not `hf_transfer`** (`HF_XET_HIGH_PERFORMANCE=1`). ~378 GB in ~8 min.
8. **NUMA auto-balancing** was enabled (AITER warns); the script disables it.

## Memory tuning (why util 0.90 + tiny prefill batch)

1M KV at fp8 needs **~46.6 GiB/GPU** (vLLM measured exactly this; matches the 576-elem ×
78-layer MLA estimate). The trap: vLLM sizes its KV pool to fill `util×192 − profiled_peak`,
but the profiled peak **under-counts** the real runtime/indexer warmup by ~6–7 GiB. So:

- **High util (0.95–0.97):** KV fills the budget → warmup/graph-capture has 0 bytes left → **OOM**.
- **Fix:** keep `util=0.90` (leaves ~8 GiB physical headroom) **and** shrink the prefill
  activation with `--max-num-batched-tokens 2048`, which frees ~27 GiB for KV. Result:
  **67 GiB KV available, 1.45× concurrency at full 1M context.** Stable at ~183/192 GiB used.
- Raising `--max-num-batched-tokens` to "help" does the opposite — it enlarges the profiled
  activation peak and shrinks KV. Counterintuitive but measured.

## Performance (measured, single stream)

| config | context | decode | TTFT (6k prompt) |
|---|---|---|---|
| `--enforce-eager` (default) | **1M** | **4.5 tok/s** | 4.1 s |
| CUDA graphs (`EAGER=0`) | 256k | **15.2 tok/s** | 2.7 s |

**CUDA graphs give a ~3.4× decode speedup** but only fit at reduced context (capture needs
memory the 1M KV pool already took). To use them:

```bash
EAGER=0 MAX_MODEL_LEN=262144 MAX_NUM_BATCHED_TOKENS=8192 ./boot_glm52.sh
```

The script auto-adds `--disable-custom-all-reduce` in graph mode — **required** on these
virtualized (MI300X VF) GPUs, because graph capture enables AITER custom all-reduce which
uses HIP IPC handles that fail on SR-IOV VFs (`hipIpcGetMemHandle -> invalid argument`).

### Why even 15 tok/s is below the hardware's potential

GLM-4.6 (similar MoE) does ~49 tok/s here. Two penalties:
- **eager vs graphs** (~3.4×) — recovered by trading 1M → 256k context.
- **non-AITER MoE** (~3×) — *not* recoverable yet: the fast AITER MoE kernel is the one that
  emits garbage (finding #2), so we're stuck on the slow correct path. Bleeding-edge (model
  released 2026-06-13); **re-test `AITER_MOE=1` on newer `rocm/vllm-dev` tags** — when that bug
  is fixed, throughput should jump toward 40–50 tok/s.

### Choosing a config
- **Max context (research/large repos), latency-tolerant:** default 1M eager, ~4.5 tok/s.
- **Interactive coding agent:** `EAGER=0` at 256k, ~15 tok/s. Usually the better trade.

## SGLang — tested, does NOT load this checkpoint (2026-06)

`lmsysorg/sglang:v0.5.14-rocm720-mi30x` **supports the GLM-5.2 architecture**
(`class GlmMoeDsaForCausalLM(DeepseekV2ForCausalLM)`, DSA via tilelang kernels, `glm47`
tool detector) — but it **crashes loading the QuantTrio Int4-Int8Mix checkpoint**:

```
deepseek_v2.py:1698  self.fused_qkv_a_proj_with_mqa.weight.dtype == torch.bfloat16
AttributeError: 'ReplicatedLinear' object has no attribute 'weight'
```

SGLang's MLA path assumes an unquantized `.weight` on the attention QKV-A projection, but in
this checkpoint that projection is W8A16 compressed-tensors quantized → no `.weight`. Not
flag-fixable. SGLang would need an unquantized/bf16 or FP8 checkpoint (neither fits on 4
GPUs), so **SGLang is not a path on 4× MI300X with this INT4 build.** It converges with the
8-GPU + FP8 recommendation. Worth re-testing on newer SGLang once compressed-tensors MLA
loading is fixed.

## Does 8× MI300X help? (vs software limit)

The ~15 tok/s ceiling is two stacked penalties: eager-vs-graphs (~3.4×, a 4-GPU **memory**
limit) and the non-AITER MoE (~3×, a **software/kernel** bug — the fast AITER MoE emits
garbage on W4A16). 8× MI300X doesn't add raw decode speed (single-stream decode is
comms/bandwidth bound and scales sublinearly), but the extra memory lets you:
1. run **1M + CUDA graphs together** (weights drop to ~47 GiB/GPU), and
2. fit the **FP8 checkpoint**, which AMD's ATOM recipe runs with the *working* AITER MoE —
   likely recovering the second ~3× and reaching ~30–50 tok/s at full 1M.

So: software-dominated, but 8 GPUs is the clean fix because it lets you switch to the
validated FP8 fast path. On 4 GPUs you're pinned to the INT4 build and its slow-but-correct
MoE kernel until the AITER W4A16 MoE bug is fixed upstream (re-test `AITER_MOE=1` periodically).

## Target machine (validated on)

Ubuntu 24.04.4, ROCm 7.2.4 host, 4× MI300X VF (gfx942, ~192 GiB each), 52 cores, 881 GiB RAM,
12 TB root fs, Docker 29.5, user in `docker`/`video`/`render` groups.

## Authentication

The server listens on `0.0.0.0:8000` and **is reachable from the internet** (verified), so
API-key auth is **ON by default**. The key is auto-generated on first run, saved to
`~/.glm52_api_key` (chmod 600), and passed to the container via `VLLM_API_KEY` (env, not argv).

```bash
# every /v1 call needs the bearer token (/health and /metrics stay open):
curl http://23.183.40.68:8000/v1/chat/completions \
  -H "Authorization: Bearer $(ssh hotaisle@23.183.40.68 cat .glm52_api_key)" \
  -H 'Content-Type: application/json' \
  -d '{"model":"glm-5.2","messages":[{"role":"user","content":"hi"}]}'
```

- Pin your own key: `API_KEY=mysecret ./boot_glm52.sh`
- Rotate: delete `~/.glm52_api_key` (or set a new `API_KEY=`) and relaunch.
- The key is the **only** barrier (port is public, no rate limiting) — keep it secret; a leak
  = open GPU time. For stronger isolation, run with `NOAUTH=0` **and** bind to loopback
  (`-p 127.0.0.1:8000:8000`) + SSH tunnel, or restrict the port with a cloud firewall.
- `NOAUTH=1 ./boot_glm52.sh` deliberately disables auth (don't, on a public port).

## Operating it

```bash
docker logs -f glm52                         # follow server
curl localhost:8000/health                   # readiness (no auth)
docker rm -f glm52                           # stop
SKIP_DOWNLOAD=1 SKIP_PULL=1 ./boot_glm52.sh  # quick relaunch (reuses saved API key)
```
