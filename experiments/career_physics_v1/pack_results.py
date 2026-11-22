#!/usr/bin/env python3
"""Lossless packaging of large native receipts; preserve original-byte digests."""
import gzip
import hashlib
import json
import pathlib

OUT = pathlib.Path(__file__).resolve().parent / "results"


def digest(data):
    return "sha256:" + hashlib.sha256(data).hexdigest()


def main():
    campaign=json.loads((OUT/"campaign-v1.json").read_text())
    files={}
    for name in ["baselines-v1.json","screen-v1.json","confirmation-v1.json"]:
        path=OUT/name
        raw=path.read_bytes()
        if digest(raw)!=campaign["files"][name]:
            raise RuntimeError(f"changed raw receipt:{name}")
        packed=gzip.compress(raw,mtime=0)
        if gzip.decompress(packed)!=raw:
            raise RuntimeError("lossy packaging")
        target=path.with_suffix(".json.gz")
        target.write_bytes(packed)
        files[target.name]={"compressed_digest":digest(packed),"original_digest":digest(raw),"original_file":name,"original_bytes":len(raw),"compressed_bytes":len(packed)}
        path.unlink()
    (OUT/"packaging-v1.json").write_text(json.dumps({"schema":"pitgun.lossless-evidence-packaging/v1","method":"gzip mtime=0; original campaign digests apply to decompressed exact bytes","files":files},indent=2)+"\n")
    print(json.dumps(files,indent=2))


if __name__=="__main__":
    main()
