#!/usr/bin/env python3
"""Scorpio problem reports: the service behind https://reports.lxxvll.com.

    reports.py serve [--db PATH] [--port N]   take reports (behind Caddy)
    reports.py list [--all]                   open reports, newest first
    reports.py show ID                        one report in full
    reports.py close ID                       mark it handled

Standard library only. A report is sent by Scorpio when the player presses
Send after reading it (docs/plans/reports.md). IP addresses are never
written down: they live in memory for the rate limit and are forgotten.
"""

import argparse
import json
import os
import secrets
import sqlite3
import sys
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

DB = os.environ.get("SCORPIO_REPORTS_DB", "/var/lib/scorpio-reports/reports.db")
PORT = 8798
MAX_BODY = 512 * 1024
FIELDS = {"version": 32, "system": 400, "description": 4000, "contact": 200, "text": 500 * 1024}
REQUIRED = ("version", "description", "text")
PER_ADDRESS = (5, 3600)
IN_ALL = (200, 86400)
KEEP_DAYS = 180
# Crockford base32 without 0/O, 1/I/L, U: easy to read aloud and retype.
ALPHABET = "23456789ABCDEFGHJKMNPQRSTVWXYZ"

SCHEMA = """
create table if not exists reports (
    id text primary key,
    received integer not null,
    version text not null,
    system text not null,
    description text not null,
    contact text not null,
    text text not null,
    closed integer not null default 0
)
"""


def connect(path):
    db = sqlite3.connect(path, timeout=10)
    db.execute("pragma journal_mode=wal")
    db.execute(SCHEMA)
    return db


def new_id():
    return "SCR-" + "".join(secrets.choice(ALPHABET) for _ in range(6))


def parse(body):
    """The report from a request body, or the reason it is refused."""
    try:
        data = json.loads(body.decode("utf-8"))
    except (UnicodeDecodeError, ValueError):
        return None, "not JSON"
    if not isinstance(data, dict):
        return None, "not a JSON object"
    report = {}
    for name, limit in FIELDS.items():
        value = data.get(name, "")
        if not isinstance(value, str):
            return None, f"{name} is not text"
        value = value.strip() if name != "text" else value
        if len(value) > limit:
            return None, f"{name} is longer than {limit} characters"
        if name in REQUIRED and not value:
            return None, f"{name} is empty"
        report[name] = value
    return report, None


class Limits:
    """Sliding windows: a few reports per address, a ceiling for everyone."""

    def __init__(self, per_address=PER_ADDRESS, in_all=IN_ALL, clock=time.monotonic):
        self.per_address, self.in_all, self.clock = per_address, in_all, clock
        self.seen, self.all = {}, []
        self.lock = threading.Lock()

    def allow(self, address):
        now = self.clock()
        with self.lock:
            self.all = [t for t in self.all if now - t < self.in_all[1]]
            for key in [k for k, times in self.seen.items() if not times or now - times[-1] >= self.per_address[1]]:
                del self.seen[key]
            mine = [t for t in self.seen.get(address, []) if now - t < self.per_address[1]]
            if len(mine) >= self.per_address[0] or len(self.all) >= self.in_all[0]:
                return False
            self.seen[address] = mine + [now]
            self.all.append(now)
            return True


def store(path, report, now=None):
    now = int(now if now is not None else time.time())
    db = connect(path)
    try:
        with db:
            db.execute("delete from reports where received < ?", (now - KEEP_DAYS * 86400,))
            while True:
                report_id = new_id()
                try:
                    db.execute(
                        "insert into reports values (?, ?, ?, ?, ?, ?, ?, 0)",
                        (report_id, now, report["version"], report["system"], report["description"], report["contact"], report["text"]),
                    )
                    return report_id
                except sqlite3.IntegrityError:
                    continue
    finally:
        db.close()


def handler(path, limits):
    class Handler(BaseHTTPRequestHandler):
        server_version = "reports"
        sys_version = ""

        def reply(self, status, payload):
            body = json.dumps(payload).encode()
            self.send_response(status)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.send_header("Cache-Control", "no-store")
            self.end_headers()
            self.wfile.write(body)

        def do_GET(self):
            if self.path == "/v1/health":
                self.reply(200, {"ok": True})
            else:
                self.reply(404, {"error": "not found"})

        def do_POST(self):
            if self.path != "/v1/reports":
                return self.reply(404, {"error": "not found"})
            try:
                length = int(self.headers.get("Content-Length", ""))
            except ValueError:
                return self.reply(411, {"error": "length required"})
            if length < 0 or length > MAX_BODY:
                return self.reply(413, {"error": "the report is too large"})
            body = self.rfile.read(length)
            report, why = parse(body)
            if why:
                return self.reply(400, {"error": why})
            # Caddy is the only client (127.0.0.1) and sets X-Forwarded-For to the caller.
            address = self.headers.get("X-Forwarded-For", self.client_address[0]).split(",")[-1].strip()
            if not limits.allow(address):
                return self.reply(429, {"error": "too many reports; try again in an hour"})
            report_id = store(path, report)
            self.log_message("report %s from Scorpio %s", report_id, report["version"])
            self.reply(201, {"id": report_id})

        def log_message(self, format, *args):
            # No addresses in the log: the request line and the outcome only.
            sys.stderr.write((format % args) + "\n")

    return Handler


def serve(path, port):
    connect(path).close()
    server = ThreadingHTTPServer(("127.0.0.1", port), handler(path, Limits()))
    server.daemon_threads = True
    print(f"taking reports on 127.0.0.1:{port}, stored in {path}", file=sys.stderr)
    server.serve_forever()


def stamp(seconds):
    return time.strftime("%Y-%m-%d %H:%M UTC", time.gmtime(seconds))


def main(argv=None):
    parser = argparse.ArgumentParser(description="Scorpio problem reports")
    parser.add_argument("--db", default=DB)
    commands = parser.add_subparsers(dest="command", required=True)
    run = commands.add_parser("serve")
    run.add_argument("--port", type=int, default=PORT)
    listing = commands.add_parser("list")
    listing.add_argument("--all", action="store_true")
    commands.add_parser("show").add_argument("id")
    commands.add_parser("close").add_argument("id")
    args = parser.parse_args(argv)

    if args.command == "serve":
        return serve(args.db, args.port)
    db = connect(args.db)
    if args.command == "list":
        rows = db.execute(
            "select id, received, version, closed, description from reports"
            + ("" if args.all else " where closed = 0")
            + " order by received desc"
        ).fetchall()
        for report_id, received, version, closed, description in rows:
            first = description.splitlines()[0][:70] if description else ""
            print(f"{report_id}  {stamp(received)}  {version:<8} {'closed' if closed else 'open  '}  {first}")
        if not rows:
            print("no reports")
    elif args.command == "show":
        row = db.execute("select * from reports where id = ?", (args.id.upper(),)).fetchone()
        if not row:
            print(f"no report {args.id}", file=sys.stderr)
            return 1
        report_id, received, version, system, description, contact, text, closed = row
        print(f"{report_id}, {stamp(received)}, {'closed' if closed else 'open'}")
        print(f"Scorpio {version} on {system}")
        print(f"Contact: {contact or '(none)'}\n\nWhat happened:\n{description}\n\n{text}")
    elif args.command == "close":
        with db:
            changed = db.execute("update reports set closed = 1 where id = ?", (args.id.upper(),)).rowcount
        print("closed" if changed else f"no report {args.id}")
        return 0 if changed else 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
