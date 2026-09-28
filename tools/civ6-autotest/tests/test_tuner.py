import re
import socket
import struct
import sys
import threading
import time
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from tuner import COMMAND, HANDSHAKE, TunerClient, TunerError, recv_frame, send_frame


class FakeTuner:
    def __init__(self, lua_error=False, no_ack=False):
        self.server = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        self.server.bind(("127.0.0.1", 0))
        self.server.listen(1)
        self.port = self.server.getsockname()[1]
        self.lua_error = lua_error
        self.no_ack = no_ack
        self.received = []
        self.thread = threading.Thread(target=self._serve, daemon=True)
        self.thread.start()

    def _send_split(self, connection, tag, payload):
        data = payload.encode() + b"\x00"
        wire = struct.pack("<Ii", len(data), tag) + data
        connection.sendall(wire[:3])
        connection.sendall(wire[3:9])
        connection.sendall(wire[9:])

    def _serve(self):
        try:
            with self.server:
                connection, _ = self.server.accept()
                with connection:
                    while True:
                        try:
                            frame = recv_frame(connection)
                        except (TunerError, OSError):
                            break
                        self.received.append(frame)
                        if frame.payload == "APP:":
                            self._send_split(connection, HANDSHAKE, "Civilization VI")
                        elif frame.payload == "LSQ:":
                            self._send_split(connection, HANDSHAKE, "FrontEnd\x00InGame\x00GameCore_Tuner")
                        elif frame.payload.startswith("CMD:"):
                            if self.no_ack:
                                time.sleep(0.35)
                            else:
                                marker = re.search(r"JAC_AUTOTEST_[0-9a-f]{32}", frame.payload).group()
                                self._send_split(connection, COMMAND, "unsolicited output")
                                self._send_split(connection, COMMAND, marker + ("|ERR|boom" if self.lua_error else "|OK|true"))
        except OSError:
            pass

    def close(self):
        self.server.close()
        self.thread.join(timeout=1)


class FramingTests(unittest.TestCase):
    def test_round_trip_fragmented_and_named_state(self):
        fake = FakeTuner()
        try:
            with TunerClient(port=fake.port, timeout=1) as client:
                self.assertEqual(client.identity, "Civilization VI")
                self.assertEqual(client.list_states(), ["FrontEnd", "InGame", "GameCore_Tuner"])
                result = client.run_lua("InGame", "return true")
                self.assertEqual(result.value, "true")
                self.assertIn("unsolicited output", result.output)
            commands = [frame for frame in fake.received if frame.tag == COMMAND]
            self.assertEqual(len(commands), 1)
            self.assertTrue(commands[0].payload.startswith("CMD:1:"))
        finally:
            fake.close()

    def test_lua_error_is_reported(self):
        fake = FakeTuner(lua_error=True)
        try:
            with TunerClient(port=fake.port, timeout=1) as client:
                with self.assertRaisesRegex(TunerError, "boom"):
                    client.run_lua("InGame", "error('boom')")
        finally:
            fake.close()

    def test_missing_ack_is_not_replayed(self):
        fake = FakeTuner(no_ack=True)
        try:
            with TunerClient(port=fake.port, timeout=0.15) as client:
                with self.assertRaisesRegex(TunerError, "may have run"):
                    client.run_lua("InGame", "return true", timeout=0.1)
            self.assertEqual(sum(frame.tag == COMMAND for frame in fake.received), 1)
        finally:
            fake.close()

    def test_reject_oversized_frame(self):
        left, right = socket.socketpair()
        try:
            right.sendall(struct.pack("<Ii", 2 * 1024 * 1024 + 1, 3))
            with self.assertRaisesRegex(TunerError, "size"):
                recv_frame(left)
        finally:
            left.close()
            right.close()

    def test_send_nul_is_rejected(self):
        left, right = socket.socketpair()
        try:
            with self.assertRaises(ValueError):
                send_frame(left, COMMAND, "bad\x00command")
        finally:
            left.close()
            right.close()


if __name__ == "__main__":
    unittest.main()
