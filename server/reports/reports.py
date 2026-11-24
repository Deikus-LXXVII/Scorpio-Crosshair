#!/usr/bin/env python3
"""Scorpio problem reports: the service behind https://reports.lxxvll.com.

    reports.py serve [--port N]   take reports (behind Caddy)
    reports.py list [--all]       open reports, newest first
    reports.py show ID            one report in full
    reports.py close ID           mark it handled

Standard library only. A report is sent by Scorpio when the player presses
Send after reading it (docs/plans/reports.md). IP addresses are never
written down: they live in memory for the rate limit and are forgotten.
Every request gets an HTTP answer after its body was read, so Caddy never
sees a broken upstream (its error log would hold the caller's address).
"""

import argparse
import ipaddress
import json
import os
import re
import secrets
import sqlite3
import sys
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

DB = "/var/lib/scorpio-reports/reports.db"
PORT = 8798
MAX_BODY = 512 * 1024
# Caddy refuses larger bodies itself (reports.caddy); up to this much is read and dropped.
MAX_DRAIN = 600 * 1000
FIELDS = {"version": 32, "system": 400, "description": 4000, "contact": 200, "text": 500 * 1024}
REQUIRED = ("version", "description", "text")
SINGLE_LINE = ("version", "system", "contact")
# Attempts per address (or IPv6 /64) an hour, good or bad; stored reports a day in all.
PER_ADDRESS = (10, 3600)
IN_ALL = (200, 86400)
BUSY = 4
KEEP_DAYS = 180
FULL_BYTES = 1024 * 1024 * 1024
# Crockford base32 without 0/O, 1/I/L, U: easy to read aloud and retype.
ALPHABET = "23456789ABCDEFGHJKMNPQRSTVWXYZ"
CONTROL = re.compile(r"[\x00-\x08\x0b-\x1f\x7f-\x9f]")
LINE_BREAKS = re.compile(r"[\r\n\t]+")

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
    db.execute("pragma secure_delete=on")
    db.execute(SCHEMA)
    return db


def purge(db, now=None):
    now = int(now if now is not None else time.time())
    with db:
        db.execute("delete from reports where received < ?", (now - KEEP_DAYS * 86400,))


def new_id():
    return "SCR-" + "".join(secrets.choice(ALPHABET) for _ in range(6))


def clean(name, value):
    """Text as it may be stored and printed: no terminal escapes, and one
    line for the fields the listing prints on one line."""
    if name in SINGLE_LINE:
        value = LINE_BREAKS.sub(" ", value)
    else:
        value = value.replace("\r\n", "\n").replace("\r", "\n")
    return CONTROL.sub("", value)


def parse(body):
    """The report from a request body, or the reason it is refused."""
    try:
        data = json.loads(body.decode("utf-8"))
    except (UnicodeDecodeError, ValueError, RecursionError):
        return None, "not JSON"
    if not isinstance(data, dict):
        return None, "not a JSON object"
    report = {}
    for name, limit in FIELDS.items():
        value = data.get(name, "")
        if not isinstance(value, str):
            return None, f"{name} is not text"
        value = clean(name, value)
        if name != "text":
            value = value.strip()
        if len(value) > limit:
            return None, f"{name} is longer than {limit} characters"
        if name in REQUIRED and not value.strip():
            return None, f"{name} is empty"
        report[name] = value
    return report, None


def address_key(address):
    """An IPv6 caller can change its address within its /64 at will."""
    try:
        ip = ipaddress.ip_address(address)
    except ValueError:
        return address
    if ip.version == 6:
        if ip.ipv4_mapped:
            return str(ip.ipv4_mapped)
        return str(ipaddress.ip_network(f"{ip}/64", strict=False))
    return str(ip)


class Limits:
    """Sliding windows: attempts per address, stored reports for everyone."""

    def __init__(self, per_address=PER_ADDRESS, in_all=IN_ALL, clock=time.monotonic):
        self.per_address, self.in_all, self.clock = per_address, in_all, clock
        self.seen, self.stored = {}, []
        self.lock = threading.Lock()

    def attempt(self, address):
        """Counts a request from `address`; False once it has used its hour."""
        key, now = address_key(address), self.clock()
        with self.lock:
            for old in [k for k, times in self.seen.items() if now - times[-1] >= self.per_address[1]]:
                del self.seen[old]
            mine = [t for t in self.seen.get(key, []) if now - t < self.per_address[1]]
            if len(mine) >= self.per_address[0]:
                return False
            self.seen[key] = mine + [now]
            return True

    def room(self):
        """Takes one of the day's places for a report; False when none is left."""
        now = self.clock()
        with self.lock:
            self.stored = [t for t in self.stored if now - t < self.in_all[1]]
            if len(self.stored) >= self.in_all[0]:
                return False
            self.stored.append(now)
            return True


def store(path, report, now=None):
    now = int(now if now is not None else time.time())
    db = connect(path)
    try:
        purge(db, now)
        with db:
            for _ in range(5):
                report_id = new_id()
                try:
                    db.execute(
                        "insert into reports values (?, ?, ?, ?, ?, ?, ?, 0)",
                        (report_id, now, report["version"], report["system"], report["description"], report["contact"], report["text"]),
                    )
                    return report_id
                except sqlite3.IntegrityError:
                    continue
        raise RuntimeError("no free report number after 5 tries")
    finally:
        db.close()


def full(path):
    """Over the ceiling in live data: pages freed by the 180-day purge count
    as room, though the file does not shrink."""
    if not os.path.exists(path):
        return False
    db = connect(path)
    try:
        pages = db.execute("pragma page_count").fetchone()[0] - db.execute("pragma freelist_count").fetchone()[0]
        return pages * db.execute("pragma page_size").fetchone()[0] > FULL_BYTES
    finally:
        db.close()


def handler(path, limits, busy=None):
    busy = busy or threading.BoundedSemaphore(BUSY)

    class Handler(BaseHTTPRequestHandler):
        server_version = "reports"
        sys_version = ""
        # A caller that stops sending is dropped after this many seconds.
        timeout = 15

        def reply(self, status, payload):
            body = json.dumps(payload).encode()
            self.send_response(status)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.send_header("Cache-Control", "no-store")
            self.end_headers()
            self.wfile.write(body)

        def chunked(self):
            return "chunked" in self.headers.get("Transfer-Encoding", "").lower()

        def length(self):
            """Content-Length, or None when there is neither it nor a chunked body."""
            if self.chunked():
                return 0
            try:
                length = int(self.headers.get("Content-Length", ""))
            except ValueError:
                return None
            return length if length >= 0 else None

        def body(self):
            """The body, read whole so the answer never races it; None if too large."""
            if self.chunked():
                data = b""
                while len(data) <= MAX_DRAIN:
                    size = int(self.rfile.readline(64).split(b";")[0].strip() or b"0", 16)
                    if size == 0:
                        self.rfile.readline(64)
                        break
                    data += self.rfile.read(min(size, MAX_DRAIN + 1 - len(data)))
                    self.rfile.readline(64)
                return data if len(data) <= MAX_BODY else None
            length = self.length() or 0
            data = self.rfile.read(min(length, MAX_DRAIN))
            return data if length <= MAX_BODY else None

        def caller(self):
            # Caddy is the only client (127.0.0.1) and sets X-Real-IP to the caller (reports.caddy).
            values = self.headers.get_all("X-Real-IP") or [self.client_address[0]]
            return values[-1].strip()

        def do_GET(self):
            if self.path == "/v1/health":
                self.reply(200, {"ok": True})
            else:
                self.reply(404, {"error": "not found"})

        def refuse_method(self):
            self.body()
            self.reply(405, {"error": "not allowed"})

        do_PUT = do_PATCH = do_DELETE = do_OPTIONS = refuse_method

        def do_POST(self):
            try:
                self.take()
            except (BrokenPipeError, ConnectionResetError, TimeoutError) as error:
                self.log_message("connection lost: %s", type(error).__name__)
            except Exception as error:  # an answer, never a dropped connection
                self.log_message("failed: %s", type(error).__name__)
                try:
                    self.reply(500, {"error": "the report could not be kept"})
                except OSError:
                    pass

        def take(self):
            if not busy.acquire(blocking=False):
                self.body()
                return self.reply(503, {"error": "busy; try again in a minute"})
            try:
                if self.length() is None:
                    return self.reply(411, {"error": "length required"})
                allowed = limits.attempt(self.caller())
                body = self.body()
                if self.path != "/v1/reports":
                    return self.reply(404, {"error": "not found"})
                if not allowed:
                    return self.reply(429, {"error": "too many reports; try again in an hour"})
                if body is None:
                    return self.reply(413, {"error": "the report is too large"})
                report, why = parse(body)
                if why:
                    return self.reply(400, {"error": why})
                if full(path) or not limits.room():
                    return self.reply(503, {"error": "not taking reports today; try again tomorrow"})
                report_id = store(path, report)
                self.log_message("report %s from Scorpio %s", report_id, report["version"])
                self.reply(201, {"id": report_id})
            finally:
                busy.release()

        def log_message(self, format, *args):
            # No addresses in the log: the request line and the outcome only.
            sys.stderr.write((format % args) + "\n")

    return Handler


class Server(ThreadingHTTPServer):
    daemon_threads = True

    def handle_error(self, request, client_address):
        # One line, no traceback: the handler already logs what went wrong.
        sys.stderr.write(f"request failed: {sys.exc_info()[0].__name__}\n")


def serve(path, port):
    db = connect(path)
    purge(db)
    db.close()
    server = Server(("127.0.0.1", port), handler(path, Limits()))
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
    if not os.path.exists(args.db):
        print(f"no reports database at {args.db}", file=sys.stderr)
        return 1
    db = connect(args.db)
    try:
        purge(db)
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
    finally:
        db.close()


if __name__ == "__main__":
    sys.exit(main())
