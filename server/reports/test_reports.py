import contextlib
import io
import json
import os
import socket
import sqlite3
import tempfile
import threading
import time
import unittest
import urllib.error
import urllib.request
from http.server import ThreadingHTTPServer
from unittest import mock

import reports

GOOD = {"version": "0.1.6", "system": "Windows 11 Pro 25H2", "description": "1% lows drop in CS2", "contact": "", "text": "Scorpio problem report\n..."}


class Server(unittest.TestCase):
    def setUp(self):
        self.folder = tempfile.TemporaryDirectory()
        self.db = os.path.join(self.folder.name, "reports.db")
        self.clock = [0.0]
        limits = reports.Limits(per_address=(3, 3600), in_all=(4, 86400), clock=lambda: self.clock[0])
        self.log = io.StringIO()
        self.patch = mock.patch("sys.stderr", self.log)
        self.patch.start()
        self.server = ThreadingHTTPServer(("127.0.0.1", 0), reports.handler(self.db, limits))
        threading.Thread(target=self.server.serve_forever, daemon=True).start()
        self.url = f"http://127.0.0.1:{self.server.server_address[1]}"

    def tearDown(self):
        self.server.shutdown()
        self.server.server_close()
        self.patch.stop()
        self.folder.cleanup()

    def post(self, payload, address="203.0.113.1", raw=None, path="/v1/reports"):
        body = raw if raw is not None else json.dumps(payload).encode()
        request = urllib.request.Request(self.url + path, data=body, method="POST", headers={"X-Real-IP": address, "Content-Type": "application/json"})
        try:
            with urllib.request.urlopen(request) as response:
                return response.status, json.loads(response.read())
        except urllib.error.HTTPError as error:
            with error:
                return error.code, json.loads(error.read())

    def stored(self):
        with contextlib.closing(reports.connect(self.db)) as db:
            return db.execute("select id, version, system, description, contact, text, closed from reports").fetchall()

    def test_a_report_is_kept_and_gets_a_readable_number(self):
        status, answer = self.post(GOOD)
        self.assertEqual(status, 201)
        self.assertRegex(answer["id"], r"^SCR-[2-9A-HJKMNP-TV-Z]{6}$")
        self.assertEqual(self.stored(), [(answer["id"], "0.1.6", GOOD["system"], GOOD["description"], "", GOOD["text"], 0)])

    def test_refused_reports_are_not_kept(self):
        cases = [
            ({**GOOD, "description": "  "}, "description is empty"),
            ({**GOOD, "version": 6}, "version is not text"),
            ({**GOOD, "contact": "x" * 201}, "contact is longer"),
            ({**GOOD, "text": " \n\t "}, "text is empty"),
        ]
        for payload, why in cases:
            got, answer = self.post(payload, address=f"198.51.100.{len(why)}")
            self.assertEqual(got, 400, why)
            self.assertIn(why, answer["error"])
        self.assertEqual(self.post(None, raw=b"{nope", address="192.0.2.1")[0], 400)
        self.assertEqual(self.post(None, raw=b"[" * 200_000 + b"]" * 200_000, address="192.0.2.2"), (400, {"error": "not JSON"}), "deep nesting is an answer, not a crash")
        self.assertEqual(self.post(None, raw=b"x" * (reports.MAX_BODY + 1), address="192.0.2.3")[0], 413)
        self.assertEqual(self.post(GOOD, path="/v1/other", address="192.0.2.4")[0], 404)
        self.assertEqual(self.stored(), [])

    def test_terminal_escapes_and_line_breaks_are_taken_out(self):
        self.post({**GOOD, "version": "0.1.6\nFAKE\x1b[31m", "system": "Win\x1b]52;c;ZGF0YQ==\x07", "description": "\x1b[2Jline one\r\nline two"})
        (_, version, system, description, *_), = self.stored()
        self.assertEqual(version, "0.1.6 FAKE[31m")
        self.assertEqual(system, "Win]52;c;ZGF0YQ==")
        self.assertEqual(description, "[2Jline one\nline two")
        self.assertNotIn("\nFAKE", self.log.getvalue(), "a version cannot start a log line of its own")

    def test_an_address_gets_three_tries_an_hour_and_everyone_four_reports_a_day(self):
        self.assertEqual([self.post(GOOD)[0] for _ in range(4)], [201, 201, 201, 429])
        self.clock[0] = 3600
        self.assertEqual(self.post(GOOD)[0], 201, "an hour later the address may send again")
        self.assertEqual(self.post(GOOD, address="198.51.100.7")[0], 503, "the day's ceiling of four is reached")
        self.clock[0] = 86400 + 3600
        self.assertEqual(self.post(GOOD, address="198.51.100.7")[0], 201)

    def test_bad_requests_use_up_the_hour_too(self):
        self.assertEqual([self.post(None, raw=b"{")[0] for _ in range(3)], [400, 400, 400])
        self.assertEqual(self.post(GOOD)[0], 429)

    def test_one_ipv6_network_is_one_caller(self):
        answers = [self.post(GOOD, address=f"2001:db8:1:2::{n}")[0] for n in range(1, 5)]
        self.assertEqual(answers, [201, 201, 201, 429])
        self.assertEqual(self.post(GOOD, address="2001:db8:1:3::1")[0], 201, "another /64 is another caller")

    def test_no_address_is_written_anywhere(self):
        self.post(GOOD, address="203.0.113.99")
        self.post(None, raw=b"{", address="203.0.113.98")
        for name in os.listdir(self.folder.name):
            with open(os.path.join(self.folder.name, name), "rb") as file:
                self.assertNotIn(b"203.0.113.9", file.read(), name)
        self.assertNotIn("203.0.113.9", self.log.getvalue())
        self.assertNotIn("127.0.0.1", self.log.getvalue())

    def test_a_caller_that_stops_sending_is_dropped(self):
        server = ThreadingHTTPServer(("127.0.0.1", 0), reports.handler(self.db, reports.Limits()))
        server.RequestHandlerClass.timeout = 1
        threading.Thread(target=server.serve_forever, daemon=True).start()
        try:
            with socket.create_connection(server.server_address) as slow:
                slow.sendall(b"POST /v1/reports HTTP/1.1\r\nHost: x\r\nContent-Length: 1000\r\n\r\n{")
                slow.settimeout(5)
                started = time.monotonic()
                answer = slow.recv(1024)
                self.assertLess(time.monotonic() - started, 4)
                self.assertTrue(answer == b"" or answer.startswith(b"HTTP/1.0 "), answer)
        finally:
            server.shutdown()
            server.server_close()


class Keeping(unittest.TestCase):
    def test_reports_older_than_180_days_are_deleted(self):
        with tempfile.TemporaryDirectory() as folder:
            db = os.path.join(folder, "r.db")
            reports.store(db, {**GOOD, "contact": "marker-old"}, now=1_000_000)
            new = reports.store(db, GOOD, now=1_000_000 + 181 * 86400)
            with contextlib.closing(reports.connect(db)) as conn:
                self.assertEqual([row[0] for row in conn.execute("select id from reports")], [new])
                conn.execute("pragma wal_checkpoint(truncate)")
            for name in os.listdir(folder):
                with open(os.path.join(folder, name), "rb") as file:
                    self.assertNotIn(b"marker-old", file.read(), "deleted text is overwritten (secure_delete)")

    def test_the_listing_deletes_old_reports_without_a_new_one(self):
        with tempfile.TemporaryDirectory() as folder:
            db = os.path.join(folder, "r.db")
            reports.store(db, GOOD, now=time.time() - 200 * 86400)
            out = io.StringIO()
            with contextlib.redirect_stdout(out):
                self.assertEqual(reports.main(["--db", db, "list", "--all"]), 0)
            self.assertEqual(out.getvalue(), "no reports\n")

    def test_list_show_and_close(self):
        with tempfile.TemporaryDirectory() as folder:
            db = os.path.join(folder, "r.db")
            out = io.StringIO()
            with contextlib.redirect_stdout(out), contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(reports.main(["--db", db, "list"]), 1, "no database yet: none is made")
                self.assertFalse(os.path.exists(db))
                report_id = reports.store(db, GOOD)
                reports.main(["--db", db, "list"])
                self.assertEqual(reports.main(["--db", db, "show", report_id]), 0)
                self.assertEqual(reports.main(["--db", db, "close", report_id.lower()]), 0)
                reports.main(["--db", db, "list"])
                reports.main(["--db", db, "list", "--all"])
                self.assertEqual(reports.main(["--db", db, "show", "SCR-NOPE22"]), 1)
                self.assertEqual(reports.main(["--db", db, "close", "SCR-NOPE22"]), 1)
            lines = out.getvalue().splitlines()
            self.assertRegex(lines[0], rf"^{report_id}  \d{{4}}-\d\d-\d\d \d\d:\d\d UTC  0.1.6    open    1% lows drop in CS2$")
            self.assertIn(f"{report_id}, ", lines[1])
            self.assertIn("Scorpio 0.1.6 on Windows 11 Pro 25H2", out.getvalue())
            self.assertIn("closed", lines)
            self.assertIn("no reports", lines, "closed reports leave the default listing")
            self.assertTrue(lines[-2].startswith(report_id) and " closed " in lines[-2], "and stay in --all")


if __name__ == "__main__":
    unittest.main()
