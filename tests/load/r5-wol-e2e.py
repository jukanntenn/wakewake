#!/usr/bin/env python3
"""R5 真实 wake 端到端(host python 版,undici fetch 对本 rig 有未解 404,curl 口径稳定)。
用法与 mjs 版一致:r5-wol-e2e.py prepare|getcode|measure"""

import json, sys, time, hashlib, urllib.request, base64, secrets
from urllib.error import HTTPError

BASE = "http://localhost:8443"
EMAIL = (
    __import__("os").environ.get("R5_EMAIL")
    or f"r5-{int(time.time())}@load.wakewake.local"
)
PASSWORD = "TestPass123!"
MAC = "AABBCC001122"
ROUNDS = int(__import__("os").environ.get("R5_ROUNDS", "10"))


def api(path, method="GET", token=None, body=None):
    req = urllib.request.Request(BASE + "/api/v1/" + path, method=method)
    req.add_header("Content-Type", "application/json")
    req.add_header(
        "X-Forwarded-For", "10.88.0.5"
    )  # R5 专用伪 IP(轮询 5 req/s 会吃满宿主 IP 的 per-IP 桶)
    if token:
        req.add_header("Authorization", "Bearer " + token)
    data = json.dumps(body).encode() if body else None
    try:
        with urllib.request.urlopen(req, data, timeout=10) as r:
            return json.loads(r.read() or b"{}")
    except HTTPError as e:
        raise SystemExit(f"HTTP {e.code} {method} {path}: {e.read().decode()[:300]}")


def solve_pow():
    ch = api("pow/challenge")
    prefix = "0" * ch["difficulty"]
    nonce = 0
    while (
        not hashlib.sha256((ch["challenge"] + str(nonce)).encode())
        .hexdigest()
        .startswith(prefix)
    ):
        nonce += 1
    return {"challenge": ch["id"], "nonce": str(nonce)}


cmd = sys.argv[1] if len(sys.argv) > 1 else ""
if cmd == "prepare":
    api(
        "auth/register",
        "POST",
        body={"email": EMAIL, "password": PASSWORD, **solve_pow()},
    )
    print(EMAIL)
elif cmd == "getcode":
    tok = api("auth/login", "POST", body={"email": EMAIL, "password": PASSWORD})[
        "access_token"
    ]
    print(api("agents/default", token=tok)["pairing_code"])
elif cmd == "measure":
    tok = api("auth/login", "POST", body={"email": EMAIL, "password": PASSWORD})[
        "access_token"
    ]
    pk = None
    for _ in range(60):
        a = api("agents/default", token=tok)
        if a.get("public_key"):
            pk = a["public_key"]
            break
        time.sleep(2)
    if not pk:
        sys.exit("agent public_key 未上报")
    from cryptography.hazmat.primitives import serialization
    from cryptography.hazmat.primitives.asymmetric import padding
    from cryptography.hazmat.primitives import hashes

    key = serialization.load_pem_public_key(pk.encode())
    mac_enc = base64.b64encode(
        key.encrypt(
            MAC.encode(),
            padding.OAEP(
                mgf=padding.MGF1(hashes.SHA256()), algorithm=hashes.SHA256(), label=None
            ),
        )
    ).decode()
    dev = api(
        "devices",
        "POST",
        token=tok,
        body={"name": "R5-Target-PC", "mac_encrypted": mac_enc, "mac_display": MAC},
    )
    time.sleep(6)
    lat = []
    fails = []
    for i in range(ROUNDS):
        t0 = time.time()
        api(f"devices/{dev['did']}/wake", "POST", token=tok)
        done = False
        for _ in range(50):
            time.sleep(0.2)
            wakes = api("wakes", token=tok)
            items = wakes if isinstance(wakes, list) else wakes.get("items", [])
            if any(
                w["created_at"] >= time.strftime("%Y-%m-%dT%H:%M:%S", time.gmtime(t0))
                or (
                    time.mktime(
                        time.strptime(w["created_at"][:19], "%Y-%m-%dT%H:%M:%S")
                    )
                    - time.timezone
                    >= t0 - 60
                )
                for w in items
            ):
                done = True
                break
        (lat if done else fails).append(time.time() - t0)
        time.sleep(0.5)
    lat.sort()
    p = lambda q: (
        f"{lat[min(len(lat) - 1, int(q * len(lat)))] * 1000:.0f}" if lat else "N/A"
    )
    print(
        f"# R5 wake 端到端报告\n\n| 参数 | 值 |\n|---|---|\n| 用户/设备 | {EMAIL} / {dev['did']} |\n| 轮次/失败 | {ROUNDS}/{len(fails)} |\n| 往返 p50/p95/p99 | {p(0.5)}/{p(0.95)}/{p(0.99)} ms |\n"
    )
else:
    sys.exit("usage: prepare|getcode|measure")
