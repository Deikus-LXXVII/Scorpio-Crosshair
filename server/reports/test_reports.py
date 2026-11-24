import json
import os
import re
import sqlite3
import tempfile
import threading
import unittest
import urllib.error
import urllib.request
from http.server import ThreadingHTTPServer

import reports

GOOD = {"version": "0.1.6", "system": "Windows 11 Pro 25H2", "description": "1% lows drop in CS2", "contact": "", "text": "Scorpio problem report\n..."}


class Server(unittest.TestCase):
    def setUp(self):
        self.folder = tempfile.TemporaryDirectory()
        self.db = os.path.join(self.folder.name, "reports.db")
        self.clock = [0.0]
        limits = reports.Limits(per_address=(2, 3600), in_all=(3, 86400), clock=lambda: self.clock[0])
        self.server = ThreadingHTTPServer(("127.0.0.1", 0), reports.handler(self.db, limits))
        threading.Thread(target=self.server.serve_forever, daemon=True).start()
        self.url = f"http://127.0.0.1:{self.server.server_address[1]}"

    def tearDown(self):
        self.server.shutdown()
        self.server.server_close()
        self.folder.cleanup()

    def post(self, payload, address="203.0.113.1", raw=None, path="/v1/reports"):
        body = raw if raw is not None else json.dumps(payload).encode()
        request = urllib.request.Request(self.url + path, data=body, method="POST", headers={"X-Forwarded-For": address, "Content-Type": "application/json"})
        try:
            with urllib.request.urlopen(request) as response:
                return response.status, json.loads(response.read())
        except urllib.error.HTTPError as error:
            return error.code, json.loads(error.read())

    def stored(self):
        return reports.connect(self.db).execute("select id, version, description, contact, text, closed from reports").fetchall()

    def test_a_report_is_kept_and_gets_a_readable_number(self):
        status, answer = self.post(GOOD)
        self.assertEqual(status, 201)
        self.assertRegex(answer["id"], r"^SCR-[2-9A-HJKMNP-TV-Z]{6}$")
        self.assertEqual(self.stored(), [(answer["id"], "0.1.6", "1% lows drop in CS2", "", GOOD["text"], 0)])

    def test_refused_reports_are_not_kept(self):
        cases = [
            ({**GOOD, "description": "  "}, 400, "description is empty"),
            ({**GOOD, "version": 6}, 400, "version is not text"),
            ({**GOOD, "contact": "x" * 201}, 400, "contact is longer"),
            ({k: v for k, v in GOOD.items() if k != "text"}, 400, "text is empty"),
        ]
        for payload, status, why in cases:
            got, answer = self.post(payload)
            self.assertEqual(got, status, why)
            self.assertIn(why, answer["error"])
        self.assertEqual(self.post(None, raw=b"{nope")[0], 400)
        self.assertEqual(self.post(None, raw=b"[1]")[0], 400)
        self.assertEqual(self.post(None, raw=b"x" * (reports.MAX_BODY + 1))[0], 413)
        self.assertEqual(self.post(GOOD, path="/v1/other")[0], 404)
        self.assertEqual(self.stored(), [])

    def test_an_address_gets_two_an_hour_and_everyone_three_a_day(self):
        self.assertEqual([self.post(GOOD)[0] for _ in range(3)], [201, 201, 429])
        self.clock[0] = 3600
        self.assertEqual(self.post(GOOD)[0], 201, "an hour later the address may send again")
        self.assertEqual(self.post(GOOD, address="198.51.100.7")[0], 429, "the day's ceiling of three is reached")
        self.clock[0] = 86400 + 3600
        self.assertEqual(self.post(GOOD, address="198.51.100.7")[0], 201)

    def test_no_address_is_written_anywhere(self):
        self.post(GOOD, address="203.0.113.99")
        with open(self.db, "rb") as file:
            self.assertNotIn(b"203.0.113.99", file.read())


class Keeping(unittest.TestCase):
    def test_reports_older_than_180_days_are_deleted_on_the_next_one(self):
        with tempfile.TemporaryDirectory() as folder:
            db = os.path.join(folder, "r.db")
            old = reports.store(db, {**GOOD, "contact": ""}, now=1_000_000)
            new = reports.store(db, {**GOOD, "contact": ""}, now=1_000_000 + 181 * 86400)
            ids = [row[0] for row in sqlite3.connect(db).execute("select id from reports")]
            self.assertEqual(ids, [new])
            self.assertNotEqual(old, new)

    def test_list_show_and_close(self):
        with tempfile.TemporaryDirectory() as folder:
            db = os.path.join(folder, "r.db")
            report_id = reports.store(db, GOOD)
            self.assertEqual(reports.main(["--db", db, "close", report_id.lower()]), 0)
            self.assertEqual(reports.main(["--db", db, "show", report_id]), 0)
            self.assertEqual(reports.main(["--db", db, "close", "SCR-NOPE22"]), 1)
            self.assertTrue(re.match(r"SCR-", report_id))


if __name__ == "__main__":
    unittest.main()
