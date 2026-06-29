#!/bin/bash

export MODEL="${MODEL:-Qwen/Qwen3.5-397B-A17B-FP8}"
export NUM_GPUS="${NUM_GPUS:-4}"
export CONTEXT_WINDOW="${CONTEXT_WINDOW:-262144}"
#VLLM_LOGGING_LEVEL=DEBUG \
VLLM_WORKER_MULTIPROC_METHOD=spawn \
PYTORCH_CUDA_ALLOC_CONF="expandable_segments:True,garbage_collection_threshold:0.8,max_split_size_mb:512" \
uv run vllm serve $MODEL --port 8000 --host 10.0.0.1 \
  --tensor-parallel-size $NUM_GPUS --max-model-len $CONTEXT_WINDOW \
  --reasoning-parser qwen3 \
  --language-model-only  --kv-cache-dtype fp8 \
  --max-num-batched-tokens 8192 --gpu-memory-utilization 0.95 \
  --enable-auto-tool-choice --tool-call-parser qwen3_coder
