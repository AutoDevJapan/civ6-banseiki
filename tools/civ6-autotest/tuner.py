"""Small, synchronous FireTuner client for a locally running Civ VI.

Protocol reference: https://github.com/lmwilki/civ6-mcp/blob/main/src/civ_mcp/tuner_client.py
This is an independent implementation of the documented framing and commands.
"""

from __future__ import annotations

import argparse
from dataclasses import dataclass
import socket
import struct
import time
import uuid


PORT = 4318
MAX_FRAME = 2 * 1024 * 1024
HANDSHAKE = 4
COMMAND = 3


class TunerError(RuntimeError):
    pass


class LuaResponseUnknown(TunerError):
    """A Lua send was attempted, but its execution/acknowledgment is unknown."""


class LuaExecutionError(TunerError):
    """FireTuner confirmed that the Lua chunk returned an ERR acknowledgment."""


@dataclass(frozen=True)
class Frame:
    tag: int
    payload: str


@dataclass(frozen=True)
class LuaResult:
    value: str
    output: str


def _read_exact(sock: socket.socket, count: int, deadline: float | None = None) -> bytes:
    parts = bytearray()
    while len(parts) < count:
        if deadline is not None:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise TunerError(f"FireTuner response timed out after {len(parts)}/{count} bytes")
            sock.settimeout(remaining)
        try:
            chunk = sock.recv(count - len(parts))
        except socket.timeout as exc:
            raise TunerError(f"FireTuner response timed out after {len(parts)}/{count} bytes") from exc
        if not chunk:
            raise TunerError("FireTuner closed the connection")
        parts.extend(chunk)
    return bytes(parts)


def send_frame(sock: socket.socket, tag: int, payload: str) -> None:
    if "\x00" in payload:
        raise ValueError("NUL is not allowed in an outgoing FireTuner command")
    body = payload.encode("utf-8") + b"\x00"
    if len(body) > MAX_FRAME:
        raise ValueError("FireTuner command is too large")
    sock.sendall(struct.pack("<Ii", len(body), tag) + body)


def recv_frame(sock: socket.socket, deadline: float | None = None) -> Frame:
    size, tag = struct.unpack("<Ii", _read_exact(sock, 8, deadline))
    if size < 1 or size > MAX_FRAME:
        raise TunerError(f"Invalid FireTuner frame size: {size}")
    return Frame(tag, _read_exact(sock, size, deadline).rstrip(b"\x00").decode("utf-8", "replace"))


def _lua_payload(index: int, code: str, token: str) -> str:
    wrapper = (
        "local __ok, __value = pcall(function()\n"
        + code
        + "\nend)\n"
        + f'print("{token}|" .. (__ok and "OK|" or "ERR|") .. tostring(__value))'
    )
    return f"CMD:{index}:{wrapper}"


# Leave room for the wrapper, acknowledgment token, a 20-digit state index and NUL.
MAX_LUA_CODE_BYTES = MAX_FRAME - len(_lua_payload(int("9" * 20), "", "JAC_AUTOTEST_" + "0" * 32).encode("utf-8")) - 1


def validate_lua_code(code: str) -> None:
    if "\x00" in code:
        raise ValueError("NUL is not allowed in Lua code")
    if len(code.encode("utf-8")) > MAX_LUA_CODE_BYTES:
        raise ValueError("Lua code is too large for a FireTuner command")


class TunerClient:
    def __init__(self, port: int = PORT, timeout: float = 5.0):
        if not 1 <= port <= 65535 or timeout <= 0:
            raise ValueError("Invalid port or timeout")
        self.port = port
        self.timeout = timeout
        self.sock: socket.socket | None = None
        self.identity = ""

    def __enter__(self) -> TunerClient:
        try:
            deadline = time.monotonic() + self.timeout
            self.sock = socket.create_connection(("127.0.0.1", self.port), self.timeout)
            self.identity = self._request("APP:", timeout=deadline - time.monotonic())
            if not self.list_states(timeout=deadline - time.monotonic()):
                raise TunerError("FireTuner returned no Lua states")
        except (OSError, TunerError) as exc:
            self.close()
            raise TunerError(f"Cannot connect to local FireTuner: {exc}") from exc
        return self

    def __exit__(self, *_args: object) -> None:
        self.close()

    def close(self) -> None:
        if self.sock is not None:
            self.sock.close()
            self.sock = None

    def _socket(self) -> socket.socket:
        if self.sock is None:
            raise TunerError("FireTuner is not connected")
        return self.sock

    def _request(self, payload: str, timeout: float | None = None) -> str:
        """Send a query and return its reply, skipping asynchronous game output frames.

        While a game is running, Lua print output arrives on the same socket as frames whose
        payload starts with "O " or "O\\x00" (verified live); those are not the reply.
        """
        sock = self._socket()
        deadline = time.monotonic() + (timeout if timeout is not None else self.timeout)
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            raise TunerError(f"No time left for {payload!r} FireTuner query")
        sock.settimeout(remaining)
        send_frame(sock, HANDSHAKE, payload)
        while True:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise TunerError(f"No reply to {payload!r} from FireTuner")
            sock.settimeout(remaining)
            frame = recv_frame(sock, deadline)
            if frame.payload[:2] in ("O ", "O\x00"):
                continue
            sock.settimeout(self.timeout)
            return frame.payload

    def list_state_pairs(self, timeout: float | None = None) -> list[tuple[int, str]]:
        """Return (state index, state name) pairs.

        The game answers LSQ with alternating fields "<index>", "<name>", ... and CMD
        needs that index, not the position of the name in the reply.
        """
        raw = self._request("LSQ:", timeout)
        if raw.startswith("LSQ:"):
            raw = raw[4:]
        parts = [part.strip() for part in raw.replace("\r", "\n").replace("\x00", "\n").split("\n") if part.strip()]
        if len(parts) >= 2 and not parts[0].lstrip("-").isdigit():
            # Older/fake servers that send names only: fall back to positions.
            return list(enumerate(parts))
        pairs: list[tuple[int, str]] = []
        for i in range(0, len(parts) - 1, 2):
            try:
                pairs.append((int(parts[i]), parts[i + 1]))
            except ValueError as exc:
                raise TunerError(f"unexpected LSQ reply near {parts[i:i + 2]!r}") from exc
        return pairs

    def list_states(self, timeout: float | None = None) -> list[str]:
        return [name for _, name in self.list_state_pairs(timeout)]

    def state_index(self, name: str, timeout: float | None = None) -> int:
        pairs = self.list_state_pairs(timeout)
        matches = [index for index, state in pairs if state.casefold() == name.casefold()]
        if len(matches) != 1:
            raise TunerError(f"Lua state {name!r} not uniquely available; states: {[n for _, n in pairs]}")
        return matches[0]

    def run_lua(self, state_name: str, code: str, timeout: float | None = None) -> LuaResult:
        """Run a chunk once and require its unique print acknowledgment.

        Never replay a command on timeout: it may already have changed the game.
        A Lua `return` in code becomes LuaResult.value; other print output is retained.
        """
        validate_lua_code(code)
        deadline = time.monotonic() + (timeout if timeout is not None else self.timeout)
        index = self.state_index(state_name, timeout=max(0.0, deadline - time.monotonic()))
        token = "JAC_AUTOTEST_" + uuid.uuid4().hex
        sock = self._socket()
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            raise TunerError(f"No time left to send Lua command to {state_name!r}")
        sock.settimeout(remaining)
        try:
            send_frame(sock, COMMAND, _lua_payload(index, code, token))
        except OSError as exc:
            # sendall may have sent some or all bytes before reporting failure.
            raise LuaResponseUnknown(f"Lua command to {state_name!r} may have run: {exc}") from exc
        lines: list[str] = []
        while True:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise LuaResponseUnknown(f"No Lua acknowledgment from {state_name!r}; command may have run. Inspect logs before retrying")
            try:
                sock.settimeout(remaining)
                frame = recv_frame(sock, deadline)
            except (TunerError, OSError) as exc:
                raise LuaResponseUnknown(f"No Lua acknowledgment from {state_name!r}; command may have run: {exc}") from exc
            lines.append(frame.payload)
            for line in frame.payload.splitlines():
                marker = token + "|"
                if marker not in line:
                    continue
                status_and_value = line.split(marker, 1)[1]
                status, separator, value = status_and_value.partition("|")
                if not separator:
                    continue
                output = "\n".join(lines)
                if status == "ERR":
                    raise LuaExecutionError(f"Lua error in {state_name}: {value}")
                if status == "OK":
                    return LuaResult(value, output)


def main() -> int:
    parser = argparse.ArgumentParser(description="Inspect a running local Civ VI FireTuner")
    parser.add_argument("command", choices=("states", "lua"))
    parser.add_argument("--state", default="InGame")
    parser.add_argument("--code", help="Lua chunk for the lua command")
    parser.add_argument("--port", type=int, default=PORT)
    args = parser.parse_args()
    if args.command == "lua" and not args.code:
        parser.error("lua requires --code")
    try:
        with TunerClient(port=args.port) as client:
            if args.command == "states":
                print(f"{client.identity}\n" + "\n".join(f"{i}: {name}" for i, name in client.list_state_pairs()))
            else:
                result = client.run_lua(args.state, args.code)
                print(result.output)
    except TunerError as exc:
        parser.exit(1, f"{exc}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
