"""Real-socket regressions for the black-box acceptance HTTP reader."""
import importlib.util
from pathlib import Path
import socket
import unittest

SPEC = importlib.util.spec_from_file_location(
    "smoke_everywhere", Path(__file__).resolve().parents[1] / "scripts/smoke_everywhere.py")
SMOKE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(SMOKE)


class EverywhereHttpTests(unittest.TestCase):
    def connection(self, response, close=False):
        client, server = socket.socketpair()
        self.addCleanup(client.close)
        self.addCleanup(server.close)
        client.settimeout(0.2)
        server.sendall(response)
        if close:
            server.shutdown(socket.SHUT_WR)
        return client

    def test_complete_response_does_not_wait_for_transport_eof(self):
        wire = b'HTTP/1.1 405 Method Not Allowed\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}'
        # The peer has delivered the entire HTTP message but has not closed TCP.
        self.assertEqual(SMOKE.read_http_response(self.connection(wire)), wire)

    def test_truncated_body_is_never_accepted_as_a_complete_rejection(self):
        wire = b'HTTP/1.1 405 Method Not Allowed\r\nContent-Length: 12\r\n\r\n{}'
        with self.assertRaises((AssertionError, EOFError, OSError)):
            SMOKE.read_http_response(self.connection(wire, close=True))

    def test_complete_empty_response_is_framed_without_eof(self):
        wire = b'HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n'
        self.assertEqual(SMOKE.read_http_response(self.connection(wire)), wire)

    def test_ambiguous_or_unbounded_framing_is_rejected(self):
        for headers in [b'', b'Content-Length: -1\r\n',
                        b'Content-Length: 2097153\r\n',
                        b'Content-Length: 2\r\nContent-Length: 2\r\n',
                        b'Content-Length: 2\r\nTransfer-Encoding: chunked\r\n']:
            with self.subTest(headers=headers), self.assertRaises((AssertionError, EOFError, OSError)):
                SMOKE.read_http_response(self.connection(b'HTTP/1.1 200 OK\r\n' + headers + b'\r\n{}', close=True))


if __name__ == '__main__':
    unittest.main()
