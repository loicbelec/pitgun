#!/usr/bin/env python3
import argparse
import socket
import struct
from datetime import datetime, timezone, timedelta

FRAME_MIN_LEN = 2 + 16 + 8  # len(u16) + ts(u128) + value(f64)
START_REAL = datetime.now(tz=timezone.utc) # real time when we started
START_TS_NS = None  # first timestamp seen

def ns_to_iso(ts_ns: int) -> str:
    global START_TS_NS
    if START_TS_NS is None:
        START_TS_NS = ts_ns  # store the first timestamp from the stream

    # Compute offset between current frame and first frame
    delta_ns = ts_ns - START_TS_NS
    delta = timedelta(microseconds=delta_ns / 1_000)  # convert ns → µs
    # Add to real start time
    real_time = START_REAL + delta

    # Format ISO string
    rem_ns = delta_ns % 1_000_000_000
    return f"{real_time.isoformat().replace('+00:00','Z')}+{rem_ns:09d}ns"

def decode_frame(data: bytes):
    """
    Layout: [len_channel: u16 LE][channel: bytes][ts_ns: u128 LE][value: f64 LE]
    Returns (channel:str, ts_ns:int, value:float) or raises ValueError.
    """
    if len(data) < FRAME_MIN_LEN:
        raise ValueError("frame too short")

    name_len = int.from_bytes(data[:2], "little")
    needed = 2 + name_len + 16 + 8
    if needed > len(data):
        raise ValueError("frame truncated")

    name_bytes = data[2:2 + name_len]
    try:
        channel = name_bytes.decode("utf-8", errors="strict")
    except UnicodeDecodeError:
        channel = name_bytes.decode("utf-8", errors="replace")

    off = 2 + name_len
    ts_ns = int.from_bytes(data[off:off + 16], "little", signed=False)
    value, = struct.unpack_from("<d", data, off + 16)
    return channel, ts_ns, value

def ns_to_iso(ts_ns: int) -> str:
    global START_TS_NS
    if START_TS_NS is None:
        START_TS_NS = ts_ns  # store the first timestamp from the stream

    # Compute offset between current frame and first frame
    delta_ns = ts_ns - START_TS_NS
    delta = timedelta(microseconds=delta_ns / 1_000)  # convert ns → µs
    # Add to real start time
    real_time = START_REAL + delta

    # Format ISO string
    rem_ns = delta_ns % 1_000_000_000
    return f"{real_time.isoformat().replace('+00:00','Z')}+{rem_ns:09d}ns"

def main():
    ap = argparse.ArgumentParser(description="Receive Pitgun emulator UDP frames")
    ap.add_argument("--host", default="127.0.0.1", help="local bind address")
    ap.add_argument("--port", type=int, default=5001, help="UDP port to bind")
    ap.add_argument("--buf", type=int, default=2048, help="receive buffer size")
    ap.add_argument("--raw", action="store_true", help="print raw bytes length too")
    args = ap.parse_args()

    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    # Allow quick rebind if needed
    sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    sock.bind((args.host, args.port))
    print(f"Listening on udp://{args.host}:{args.port}")

    count = 0
    try:
        while True:
            data, addr = sock.recvfrom(args.buf)
            try:
                channel, ts_ns, value = decode_frame(data)
                count += 1
                ts_iso = ns_to_iso(ts_ns)
                line = f"{count:06d} | {addr[0]}:{addr[1]} | {channel:<24} | ts={ts_ns} ({ts_iso}) | value={value}"
                if args.raw:
                    line += f" | bytes={len(data)}"
                print(line)
            except ValueError as e:
                print(f"[WARN] bad frame from {addr}: {e}; len={len(data)}")
    except KeyboardInterrupt:
        print("\nBye.")

if __name__ == "__main__":
    main()