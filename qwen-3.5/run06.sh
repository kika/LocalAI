export MODEL="Qwen/Qwen3-0.6B" \
VLLM_WORKER_MULTIPROC_METHOD=spawn \
PYTORCH_CUDA_ALLOC_CONF="expandable_segments:True,garbage_collection_threshold:0.8,max_split_size_mb:512" \
#VLLM_LOGGING_LEVEL=DEBUG \
uv run vllm serve $MODEL --port 8000 \
  --tensor-parallel-size 1 --max-model-len 32768 --reasoning-parser qwen3 \
  --language-model-only  --kv-cache-dtype fp8 \
  --max-num-batched-tokens 8192 --gpu-memory-utilization 0.85
