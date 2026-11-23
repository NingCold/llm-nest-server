"""Exercise registration and chat over loopback with a disposable PostgreSQL database."""
import json
import os
from pathlib import Path
import re
import secrets
import socket
import subprocess
import sys
import tempfile
import time
import urllib.error
import uuid

from cloud_acceptance import Client


def smoke(binary, root, db):
    web = root / "web"
    web.mkdir()
    (web / "index.html").write_text("chat smoke", encoding="utf-8")
    outbox = root / "outbox"
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        addr = f"127.0.0.1:{sock.getsockname()[1]}"
    origin = "http://" + addr
    env = {**os.environ, "CLOUD_DATABASE_URL": db, "CLOUD_BIND": addr, "CLOUD_PUBLIC_URL": origin,
           "CLOUD_WEB_DIR": str(web), "CLOUD_MAIL_OUTBOX": str(outbox),
           "CLOUD_ATTACHMENTS_DIR": str(root / "attachments"), "CLOUD_REQUIRE_ENCRYPTION": "false"}
    process = subprocess.Popen([binary], env=env)
    try:
        clients = [Client(origin, None) for _ in range(2)]
        for attempt in range(150):
            if process.poll() is not None:
                raise RuntimeError(f"Server exited on startup: {process.returncode}")
            try:
                assert clients[0].request("/health") == b"ok"
                break
            except urllib.error.URLError:
                if attempt == 149:
                    raise
                time.sleep(0.2)
        assert clients[0].request("/") == b"chat smoke"
        ids = []
        for client in clients:
            email = f"chat-{uuid.uuid4().hex}@example.invalid"
            invite = subprocess.check_output([binary, "invite"], env=env, text=True).strip()
            client.request("/api/auth/register", "POST", {"invite": invite, "email": email,
                           "password": secrets.token_urlsafe(24), "name": "Chat smoke"})
            letters = (json.loads(p.read_text(encoding="utf-8")) for p in outbox.glob("*.json"))
            letter = next(letter for letter in letters if letter["to"] == email)
            token = re.search(r"token=([a-f0-9]+)", letter["body"])[1]
            ids.append(client.request("/api/auth/verify", "POST", {"token": token})["account"]["id"])
        first, second = clients
        chat = first.request("/api/chat/direct", "POST", {"accountId": ids[1]})["id"]
        assert second.request("/api/chat/direct", "POST", {"accountId": ids[0]})["id"] == chat
        path = f"/api/chat/conversations/{chat}/messages"
        message = {"requestId": str(uuid.uuid4()), "text": "Hello from chat smoke"}
        sent = first.request(path, "POST", message)["id"]
        assert first.request(path, "POST", message)["id"] == sent
        assert [row["id"] for row in second.request(path)["messages"]] == [sent]
        print("PASS: server boot, Web, two registrations, shared direct chat, send/retry and delivery")
    finally:
        if process.poll() is None:
            process.terminate()
        process.wait(timeout=10)


if __name__ == "__main__":
    with tempfile.TemporaryDirectory() as tmp:
        smoke(Path(sys.argv[1]).resolve(), Path(tmp), os.environ["CLOUD_TEST_DATABASE_URL"])
