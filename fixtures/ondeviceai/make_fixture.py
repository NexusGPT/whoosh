#!/usr/bin/env python3
"""Builds the fake language-model files the install-step check downloads. Deterministic; nothing here is a real model.

    python3 fixtures/ondeviceai/make_fixture.py [outdir]    default: fixtures/ondeviceai/served (git-ignored)

Writes three folders and one spec per variant:
  served/good/       a valid safetensors file (header + one tensor) and three small config files
  served/tampered/   the same names with one byte of the weights changed, so the pinned checksum no longer matches
  served/damaged/   weights with a broken header whose checksum IS pinned in spec-damaged.json: installs, then fails the load check
Each spec-<variant>.json is the file list the app is told to expect: [{path, bytes, sha256}].
"""
import hashlib, json, os, struct, sys

WEIGHT_BYTES = 6 * 1024 * 1024          # big enough to cancel and resume against a throttled server
ROOT = sys.argv[1] if len(sys.argv) > 1 else os.path.join(os.path.dirname(os.path.abspath(__file__)), "served")


def weights(damaged=False):
    n = WEIGHT_BYTES
    header = json.dumps({"w": {"dtype": "U8", "shape": [n], "data_offsets": [0, n]}}).encode()
    body = bytes((i * 31 + 7) & 0xFF for i in range(n))
    if damaged:
        return struct.pack("<Q", 7_000_000_000_000) + header + body      # header length far beyond the file
    return struct.pack("<Q", len(header)) + header + body


SMALL = {"config.json": b'{"model_type": "fixture"}\n', "tokenizer.json": b'{"fixture": true}\n', "tokenizer_config.json": b'{"fixture": true}\n'}


def write(folder, files):
    os.makedirs(os.path.join(ROOT, folder), exist_ok=True)
    for name, data in files.items():
        open(os.path.join(ROOT, folder, name), "wb").write(data)


def spec(files):
    order = ["config.json", "tokenizer_config.json", "tokenizer.json", "model.safetensors"]
    return [{"path": n, "bytes": len(files[n]), "sha256": hashlib.sha256(files[n]).hexdigest()} for n in order]


good = dict(SMALL, **{"model.safetensors": weights()})
tampered = dict(good); w = bytearray(good["model.safetensors"]); w[-1] ^= 0xFF; tampered["model.safetensors"] = bytes(w)
damaged = dict(SMALL, **{"model.safetensors": weights(damaged=True)})
write("good", good); write("tampered", tampered); write("damaged", damaged)
json.dump(spec(good), open(os.path.join(ROOT, "spec-good.json"), "w"), indent=1)
json.dump(spec(damaged), open(os.path.join(ROOT, "spec-damaged.json"), "w"), indent=1)
print("ok", ROOT)
