from __future__ import annotations

import hashlib
import struct
from typing import Protocol


class Embedder(Protocol):
    def embed(self, text: str) -> list[float]:
        ...


class FakeEmbedder:
    """Deterministic hash-based embedding for tests. No model, no network."""
    dim: int = 16

    def embed(self, text: str) -> list[float]:
        # Map sha512 bytes to finite floats via unsigned ints, NOT IEEE-754
        # bit-reinterpretation (which yields NaN/inf for ~5% of inputs).
        h = hashlib.sha512(text.encode()).digest()
        floats = [
            struct.unpack_from(">I", h, i * 4)[0] / 0xFFFFFFFF - 0.5
            for i in range(self.dim)
        ]
        norm = sum(x * x for x in floats) ** 0.5 or 1.0
        return [x / norm for x in floats]


class OnnxEmbedder:
    """Local nomic-embed-text via ONNX. Raises FileNotFoundError if model absent."""
    def __init__(self, model_path: str):
        import onnxruntime as ort  # type: ignore
        self._sess = ort.InferenceSession(model_path)

    def embed(self, text: str) -> list[float]:
        raise NotImplementedError("OnnxEmbedder.embed: wire up tokenizer + session")
