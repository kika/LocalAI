#!/usr/bin/env bash
#
# boot_glm52.sh — Boot GLM-5.2 (4-bit) as a single-user, 1M-context coding-agent
# server on a 4x AMD MI300X box (ROCm) using vLLM.
#
# Designed for: Ubuntu 24.04, ROCm 7.x host, 4x MI300X (gfx942, ~192 GiB each),
# Docker present, user in docker/video/render groups.
#
# The script is idempotent: re-running skips work already done (model download,
# image pull) and recreates the server container. Tune via the env vars below.
#
# Stages:  preflight -> hf cli -> model download -> image pull -> arch check -> serve -> smoke test
#
set -euo pipefail

# ----------------------------- configuration --------------------------------
# Override any of these by exporting before running, e.g.  PORT=9000 ./boot_glm52.sh
MODEL_REPO="${MODEL_REPO:-QuantTrio/GLM-5.2-Int4-Int8Mix}"   # ~378 GB, W4A16/W8A16 compressed-tensors
MODEL_DIR="${MODEL_DIR:-$HOME/models/GLM-5.2-Int4-Int8Mix}"  # lives on the 12 TB root fs
SERVED_NAME="${SERVED_NAME:-glm-5.2}"
# IMPORTANT: GLM-5.2 uses the brand-new `glm_moe_dsa` arch. As of 2026-06 the STABLE
# rocm/vllm:latest does NOT support it (only GLM-4.5/4.6). The nightly dev image does.
# Leave IMAGE empty to auto-pick the first candidate whose vLLM supports the arch.
IMAGE="${IMAGE:-}"
IMAGE_CANDIDATES="${IMAGE_CANDIDATES:-rocm/vllm-dev:nightly rocm/vllm:latest}"
CONTAINER="${CONTAINER:-glm52}"
PORT="${PORT:-8000}"

TP="${TP:-4}"                          # tensor-parallel across all 4 GPUs
MAX_MODEL_LEN="${MAX_MODEL_LEN:-1048576}"   # full 1M context
KV_CACHE_DTYPE="${KV_CACHE_DTYPE:-fp8}"     # fp8 halves KV vs bf16; set bf16 if you see drift
# Memory model (learned empirically on this box, 4-bit weights = ~94.5 GiB/GPU):
#   vLLM sizes its KV pool to fill (util*192 - profiled_peak). The profiled peak
#   UNDER-counts the real runtime/indexer warmup by ~6-7 GiB, so if KV fills the whole
#   budget, that warmup overflows the physical ceiling -> OOM (seen at util 0.95 & 0.97).
# Fix: keep util MODERATE (0.90) so ~8 GiB physical headroom remains, and SHRINK the
#   prefill activation with a small --max-num-batched-tokens so the 1M KV (~46.6 GiB)
#   still fits inside that budget. Small batch frees ~27 GiB of activation -> KV ~66 GiB.
GPU_MEM_UTIL="${GPU_MEM_UTIL:-0.90}"
# Small prefill chunk: frees activation memory for the 1M KV pool AND leaves physical
# headroom. 1M context prefills in 2048-token chunks (fine for single-user; prefix
# caching covers repeated context). Raise for faster cold-prefill IF memory allows.
MAX_NUM_BATCHED_TOKENS="${MAX_NUM_BATCHED_TOKENS:-2048}"
MAX_NUM_SEQS="${MAX_NUM_SEQS:-4}"      # single user; small batch keeps KV for the long seq
# CUDA-graph capture OOMs at 1M ctx on 4x MI300X (KV fills the budget, no room left to
# capture). enforce-eager skips graph capture AND torch.compile -> reliable boot, faster
# startup, at the cost of somewhat slower decode. To try graphs instead, set EAGER=0 and
# optionally CUDA_GRAPH_SIZES="1 2 4" to shrink capture memory for single-user batches.
EAGER="${EAGER:-1}"
CUDA_GRAPH_SIZES="${CUDA_GRAPH_SIZES:-}"
# CRITICAL: the AITER fused-MoE kernel produces GARBAGE output for this W4A16 checkpoint
# on gfx942 (MI300X). Keep AITER on (required for GLM-5.2 sparse attention) but route the
# MoE through the non-AITER path. Without this, the server runs but emits gibberish.
AITER_MOE="${AITER_MOE:-0}"
TOOL_PARSER="${TOOL_PARSER:-glm47}"    # GLM-5.2 tool-call format (per official vLLM recipe)
REASONING_PARSER="${REASONING_PARSER:-glm45}"  # GLM-5.2 reasoning/thinking parser
# MTP speculative decoding would be a nice single-user latency win, BUT the
# QuantTrio Int4-Int8Mix checkpoint's MTP-block experts are not group-quantized,
# and vLLM's WNA16 MoE method asserts strategy=="group" -> startup crash.
# So default OFF for this checkpoint. To use MTP, serve an FP8 checkpoint (8 GPUs)
# or a 4-bit build whose MTP experts use group-strategy quant.
MTP="${MTP:-0}"
MTP_TOKENS="${MTP_TOKENS:-3}"
HF_TOKEN="${HF_TOKEN:-}"               # only needed if you switch to a gated repo

# API-key auth. The server listens on 0.0.0.0:${PORT} (publicly reachable), so a key is the
# only barrier — it is ON by default. The key is passed via the VLLM_API_KEY env (not argv,
# so it doesn't show in `ps`). Resolution order: explicit API_KEY env > persisted key file >
# auto-generate + persist (printed once). Set NOAUTH=1 to deliberately run with no auth.
API_KEY="${API_KEY:-}"
KEY_FILE="${KEY_FILE:-$HOME/.glm52_api_key}"
NOAUTH="${NOAUTH:-0}"

SKIP_DOWNLOAD="${SKIP_DOWNLOAD:-0}"
SKIP_PULL="${SKIP_PULL:-0}"
SERVE="${SERVE:-1}"                    # set 0 to prep everything but not launch

VENV="$HOME/.venv-hf"
LOG_PREFIX="[boot-glm52]"

log()  { echo -e "\033[1;36m${LOG_PREFIX}\033[0m $*"; }
warn() { echo -e "\033[1;33m${LOG_PREFIX} WARN:\033[0m $*" >&2; }
die()  { echo -e "\033[1;31m${LOG_PREFIX} ERROR:\033[0m $*" >&2; exit 1; }

# ------------------------------ stage: preflight ----------------------------
preflight() {
  log "Preflight checks…"
  command -v docker  >/dev/null || die "docker not found"
  command -v rocm-smi >/dev/null || die "rocm-smi not found (no ROCm host stack?)"
  [ -e /dev/kfd ] || die "/dev/kfd missing — amdgpu/kfd not available to this VM"
  [ -d /dev/dri ] || die "/dev/dri missing"

  local ngpu
  ngpu=$(rocm-smi --showproductname 2>/dev/null | grep -c 'Card Series' || true)
  log "GPUs visible to host: ${ngpu}"
  [ "${ngpu}" -ge "${TP}" ] || die "need >= ${TP} GPUs, found ${ngpu}"

  docker info >/dev/null 2>&1 || die "cannot talk to docker daemon (is user in 'docker' group?)"

  # AMD MI300X best practice: NUMA auto-balancing can cause errors/perf loss with AITER.
  if [ "$(cat /proc/sys/kernel/numa_balancing 2>/dev/null || echo 0)" = "1" ]; then
    log "Disabling NUMA auto-balancing (MI300X best practice)…"
    sudo sh -c 'echo 0 > /proc/sys/kernel/numa_balancing' 2>/dev/null || warn "could not disable NUMA balancing"
  fi

  # Disk: need ~378 GB for weights + image headroom; require ~500 GB free on MODEL_DIR fs.
  local avail_kb
  avail_kb=$(df -P "$(dirname "${MODEL_DIR}")" 2>/dev/null | awk 'NR==2{print $4}')
  avail_kb="${avail_kb:-0}"
  if [ "${avail_kb}" -lt $((500*1024*1024)) ]; then
    warn "Less than 500 GB free where the model will live; download may fail."
  fi
  log "Preflight OK."
}

# ------------------------------ stage: hf cli -------------------------------
ensure_hf_cli() {
  if [ -x "${VENV}/bin/hf" ]; then
    log "hf CLI already present (${VENV})."
    return
  fi
  log "Installing Hugging Face CLI into a venv (Ubuntu 24.04 is PEP668-managed)…"
  # Stock Ubuntu 24.04 lacks ensurepip for venv; install python3-venv if needed.
  if ! python3 -m venv --help >/dev/null 2>&1 || ! python3 -c 'import ensurepip' 2>/dev/null; then
    log "  Installing python3-venv via apt…"
    sudo apt-get update -qq
    sudo DEBIAN_FRONTEND=noninteractive apt-get install -y -qq "python3-venv" "python$(python3 -c 'import sys;print(f"{sys.version_info.major}.{sys.version_info.minor}")')-venv" || \
      sudo DEBIAN_FRONTEND=noninteractive apt-get install -y -qq python3-venv
  fi
  rm -rf "${VENV}"
  python3 -m venv "${VENV}"
  # shellcheck disable=SC1091
  "${VENV}/bin/pip" install --quiet --upgrade pip
  # huggingface_hub >=1.x uses Xet for high-performance parallel transfer (hf_transfer is deprecated).
  "${VENV}/bin/pip" install --quiet "huggingface_hub>=1.0"
  log "hf CLI installed."
}

# --------------------------- stage: model download --------------------------
download_model() {
  if [ "${SKIP_DOWNLOAD}" = "1" ]; then log "SKIP_DOWNLOAD=1 — skipping download."; return; fi
  # Consider it complete if config.json + all 124 shards are present.
  local nshards
  nshards=$(ls "${MODEL_DIR}"/model-*.safetensors 2>/dev/null | wc -l | tr -d ' ')
  if [ -f "${MODEL_DIR}/config.json" ] && [ "${nshards}" -ge 124 ]; then
    log "Model already downloaded (${nshards} shards present)."
    return
  fi
  log "Downloading ${MODEL_REPO} -> ${MODEL_DIR} (~378 GB; resumable)…"
  mkdir -p "${MODEL_DIR}"
  export HF_XET_HIGH_PERFORMANCE=1     # parallel Xet transfer (~saturates the link)
  [ -n "${HF_TOKEN}" ] && export HF_TOKEN
  # hf download is resumable; safe to re-run after interruption.
  "${VENV}/bin/hf" download "${MODEL_REPO}" \
    --local-dir "${MODEL_DIR}" \
    --exclude "original/*" "*.pth" \
    ${HF_TOKEN:+--token "${HF_TOKEN}"}
  log "Download complete."
}

# Probe one image: pull if missing, return 0 only if its vLLM supports the arch.
probe_image() {
  local img="$1"
  docker image inspect "${img}" >/dev/null 2>&1 || {
    [ "${SKIP_PULL}" = "1" ] && return 1
    log "  Pulling ${img} (large, tens of GB)…"
    docker pull "${img}" >/dev/null 2>&1 || { warn "  pull of ${img} failed"; return 1; }
  }
  local out
  out=$(docker run --rm --entrypoint python3 "${img}" -c '
from vllm.model_executor.models.registry import ModelRegistry
print("GlmMoeDsaForCausalLM" in set(ModelRegistry.get_supported_archs()))
' 2>/dev/null | tail -1)
  log "  ${img}: arch_supported=${out:-unknown}"
  [ "${out}" = "True" ]
}

# ------------------ stage: image pull + arch auto-selection ------------------
# glm_moe_dsa is brand-new. Pick the first candidate image whose vLLM supports it.
select_image() {
  if [ -n "${IMAGE}" ]; then
    log "IMAGE pinned to ${IMAGE}; verifying arch support…"
    probe_image "${IMAGE}" || warn "${IMAGE} may not support GlmMoeDsaForCausalLM — serve could fail."
    return
  fi
  log "Selecting a vLLM image that supports GlmMoeDsaForCausalLM…"
  for cand in ${IMAGE_CANDIDATES}; do
    if probe_image "${cand}"; then
      IMAGE="${cand}"
      log "Selected image: ${IMAGE}"
      return
    fi
  done
  die "No candidate image supports GlmMoeDsaForCausalLM (tried: ${IMAGE_CANDIDATES}). Update IMAGE_CANDIDATES with a newer rocm/vllm-dev tag or AMD's ATOM image."
}

# ------------------------------- stage: serve -------------------------------
serve() {
  [ "${SERVE}" = "1" ] || { log "SERVE=0 — prepared but not launching."; return; }

  log "Removing any existing '${CONTAINER}' container…"
  docker rm -f "${CONTAINER}" >/dev/null 2>&1 || true

  # Resolve API key (auto-generate + persist on first run unless NOAUTH=1).
  local auth_env=()
  if [ "${NOAUTH}" = "1" ]; then
    warn "NOAUTH=1 — server will run WITHOUT authentication (public port, anyone can call it)."
  else
    if [ -z "${API_KEY}" ] && [ -f "${KEY_FILE}" ]; then API_KEY="$(cat "${KEY_FILE}")"; fi
    if [ -z "${API_KEY}" ]; then
      API_KEY="$(openssl rand -hex 24 2>/dev/null || head -c24 /dev/urandom | od -An -tx1 | tr -d ' \n')"
      (umask 077; printf '%s' "${API_KEY}" > "${KEY_FILE}")
      log "Generated API key and saved to ${KEY_FILE}"
    fi
    auth_env=(-e "VLLM_API_KEY=${API_KEY}")
    log "API-key auth ENABLED. Clients must send:  Authorization: Bearer <key>"
    log "  key (also in ${KEY_FILE}):  ${API_KEY}"
  fi

  # Optional MTP speculative decoding (single-user latency win).
  local spec_args=()
  if [ "${MTP}" = "1" ]; then
    spec_args=(--speculative-config "{\"method\":\"mtp\",\"num_speculative_tokens\":${MTP_TOKENS}}")
    log "MTP speculative decoding ON (${MTP_TOKENS} tokens)."
  fi

  # CUDA-graph control: eager (no capture, reliable at 1M) vs graphs (~3.4x faster decode).
  # Graphs only fit at reduced context (e.g. 256k) and REQUIRE --disable-custom-all-reduce:
  # graph capture turns on AITER custom all-reduce, which uses HIP IPC mem handles that fail
  # on these virtualized SR-IOV (MI300X VF) GPUs (hipIpcGetMemHandle -> invalid argument).
  local graph_args=()
  if [ "${EAGER}" = "1" ]; then
    graph_args=(--enforce-eager)
    log "CUDA graphs OFF (--enforce-eager): reliable boot at 1M ctx, slower decode (~4.5 tok/s)."
  else
    local sizes="${CUDA_GRAPH_SIZES:-1 2 4}"
    local sizes_json="[$(echo "${sizes}" | tr ' ' ',')]"
    graph_args=(--compilation-config "{\"cudagraph_capture_sizes\":${sizes_json}}" --disable-custom-all-reduce)
    log "CUDA graphs ON (sizes: ${sizes}, custom all-reduce off for VF GPUs): ~15 tok/s, needs reduced ctx."
  fi

  log "Launching vLLM server '${CONTAINER}' on port ${PORT} (TP=${TP}, ctx=${MAX_MODEL_LEN})…"
  docker run -d \
    --name "${CONTAINER}" \
    --restart unless-stopped \
    --device=/dev/kfd --device=/dev/dri \
    --group-add video --group-add render \
    --ipc=host --shm-size=64g \
    --security-opt seccomp=unconfined \
    -p "${PORT}:8000" \
    -v "${MODEL_DIR}:/model:ro" \
    -e VLLM_ROCM_USE_AITER=1 \
    -e VLLM_ROCM_USE_AITER_MOE="${AITER_MOE}" \
    -e HF_HUB_OFFLINE=1 \
    -e PYTORCH_CUDA_ALLOC_CONF=expandable_segments:True \
    "${auth_env[@]}" \
    "${IMAGE}" \
    vllm serve /model \
      --served-model-name "${SERVED_NAME}" \
      --tensor-parallel-size "${TP}" \
      --enable-expert-parallel \
      --max-model-len "${MAX_MODEL_LEN}" \
      --kv-cache-dtype "${KV_CACHE_DTYPE}" \
      --enable-prefix-caching \
      --max-num-seqs "${MAX_NUM_SEQS}" \
      ${MAX_NUM_BATCHED_TOKENS:+--max-num-batched-tokens "${MAX_NUM_BATCHED_TOKENS}"} \
      --gpu-memory-utilization "${GPU_MEM_UTIL}" \
      --enable-auto-tool-choice \
      --tool-call-parser "${TOOL_PARSER}" \
      --reasoning-parser "${REASONING_PARSER}" \
      "${spec_args[@]}" \
      "${graph_args[@]}" \
      --host 0.0.0.0 --port 8000

  log "Container started. Model load + first-run kernel compile can take several minutes."
  log "Follow logs:  docker logs -f ${CONTAINER}"
}

# --------------------------- stage: health / smoke --------------------------
wait_healthy() {
  [ "${SERVE}" = "1" ] || return 0
  log "Waiting for the server to become ready (up to 20 min)…"
  local deadline=$(( $(date +%s) + 1200 ))
  while [ "$(date +%s)" -lt "${deadline}" ]; do
    if ! docker ps --format '{{.Names}}' | grep -q "^${CONTAINER}$"; then
      docker logs --tail 40 "${CONTAINER}" 2>&1 || true
      die "Container exited during startup — see logs above."
    fi
    # /health needs no auth; use it for readiness so this works with or without a key.
    if curl -fsS "http://localhost:${PORT}/health" >/dev/null 2>&1; then
      log "Server is READY."
      smoke_test
      return 0
    fi
    sleep 10
  done
  warn "Timed out waiting for readiness. Check: docker logs -f ${CONTAINER}"
  return 1
}

smoke_test() {
  local auth_hdr=()
  [ -n "${API_KEY}" ] && [ "${NOAUTH}" != "1" ] && auth_hdr=(-H "Authorization: Bearer ${API_KEY}")
  log "Smoke test: 16-token completion…"
  curl -fsS "http://localhost:${PORT}/v1/chat/completions" \
    -H 'Content-Type: application/json' "${auth_hdr[@]}" \
    -d "{\"model\":\"${SERVED_NAME}\",\"messages\":[{\"role\":\"user\",\"content\":\"Reply with exactly: BOOT OK\"}],\"max_tokens\":16,\"temperature\":0}" \
    | python3 -c 'import sys,json; d=json.load(sys.stdin); print("  ->", d["choices"][0]["message"]["content"].strip())' \
    || warn "Smoke test failed (server may still be warming up)."
}

# --------------------------------- main -------------------------------------
main() {
  log "=== Booting GLM-5.2 on 4x MI300X ==="
  preflight
  ensure_hf_cli
  download_model
  select_image
  serve
  wait_healthy
  log "=== Done. OpenAI-compatible API at http://<host>:${PORT}/v1  (model: ${SERVED_NAME}) ==="
}

main "$@"
