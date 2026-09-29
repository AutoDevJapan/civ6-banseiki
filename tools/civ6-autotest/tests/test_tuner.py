import re
import socket
import struct
import sys
import threading
import time
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from tuner import COMMAND, HANDSHAKE, LuaExecutionError, LuaResponseUnknown, TunerClient, TunerError, recv_frame, send_frame


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
                            self._send_split(connection, HANDSHAKE,
                                             "3\x00FrontEnd\x0017\x00InGame\x0041\x00GameCore_Tuner")
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
                self.assertEqual(client.list_state_pairs(),
                                 [(3, "FrontEnd"), (17, "InGame"), (41, "GameCore_Tuner")])
                self.assertEqual(client.list_states(), ["FrontEnd", "InGame", "GameCore_Tuner"])
                result = client.run_lua("InGame", "return true")
                self.assertEqual(result.value, "true")
                self.assertIn("unsolicited output", result.output)
            commands = [frame for frame in fake.received if frame.tag == COMMAND]
            self.assertEqual(len(commands), 1)
            self.assertTrue(commands[0].payload.startswith("CMD:17:"))
        finally:
            fake.close()

    def test_lua_error_is_reported(self):
        fake = FakeTuner(lua_error=True)
        try:
            with TunerClient(port=fake.port, timeout=1) as client:
                with self.assertRaisesRegex(LuaExecutionError, "boom"):
                    client.run_lua("InGame", "error('boom')")
            self.assertEqual(sum(frame.tag == COMMAND for frame in fake.received), 1)
        finally:
            fake.close()

    def test_missing_ack_is_not_replayed(self):
        fake = FakeTuner(no_ack=True)
        try:
            with TunerClient(port=fake.port, timeout=0.15) as client:
                with self.assertRaisesRegex(LuaResponseUnknown, "may have run"):
                    client.run_lua("InGame", "return true", timeout=0.1)
            self.assertEqual(sum(frame.tag == COMMAND for frame in fake.received), 1)
        finally:
            fake.close()

    def test_missing_state_fails_before_lua_send(self):
        fake = FakeTuner()
        try:
            with TunerClient(port=fake.port, timeout=1) as client:
                with self.assertRaisesRegex(TunerError, "not uniquely available") as caught:
                    client.run_lua("Missing", "return true")
                self.assertNotIsInstance(caught.exception, LuaResponseUnknown)
            self.assertEqual(sum(frame.tag == COMMAND for frame in fake.received), 0)
        finally:
            fake.close()

    def test_send_failure_is_classified_as_unknown(self):
        fake = FakeTuner()
        original_send = send_frame

        def fail_command_send(sock, tag, payload):
            if tag == COMMAND:
                raise OSError("send failed after possible partial write")
            return original_send(sock, tag, payload)

        try:
            with patch("tuner.send_frame", side_effect=fail_command_send):
                with TunerClient(port=fake.port, timeout=1) as client:
                    with self.assertRaisesRegex(LuaResponseUnknown, "may have run"):
                        client.run_lua("InGame", "return true")
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

    def test_empty_frame_is_skipped_not_fatal(self):
        left, right = socket.socketpair()
        try:
            right.sendall(struct.pack("<Ii", 0, 3))
            frame = recv_frame(left)
            self.assertEqual(frame.payload, "")
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
