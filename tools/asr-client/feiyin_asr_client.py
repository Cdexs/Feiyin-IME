#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""飞音识别服务 · Python 参考客户端（ASR-SERVER-491 S11）

契约：docs/ASR-PROTOCOL.md（下称“文档”）。只用 Python 标准库，自带最小 WebSocket(RFC 6455) 实现。
代码里的 `# §x.y` 注释指向文档章节。它同时是服务端的端到端冒烟测试。

用法示例
--------
1) 用连接码（二维码的文字版）识别一个 16k 单声道 16 位 WAV：
    py -3 feiyin_asr_client.py --code "feiyin-asr://pair?v=1&lan=192.168.1.5:7860&key=ABCD...&fp=3f9a..." --wav a.wav

2) 手输地址 + key + 指纹，只走外网，裸 PCM，不等实时节奏：
    py -3 feiyin_asr_client.py --wan asr.example.com:7860 --key ABCD-EFGH-... --fp 3f9a... --mode wan --pcm a.pcm --fast

3) 指纹未知：先只做 TLS 握手取回证书指纹（不发 key），与服务端设置页的指纹核对后再填 --fp：
    py -3 feiyin_asr_client.py --lan 192.168.1.5 --probe

4) 带参数 / 机器可读输出：
    py -3 feiyin_asr_client.py --code "..." --wav a.wav --translate --no-itn --vocab 飞音,LiveKit --json

退出码
------
    0  成功（收到 task-finished）
    2  参数 / 连接码错误（含手输缺指纹却不是 --probe：§1.2 未经确认的指纹不得带 key 连接）
    3  收到 task-failed（错误码打印到 stderr）
    4  升级握手被拒（HTTP 401 InvalidKey/KeyExpired/KeyRevoked，503 ServicePaused，或其它非 101）
    5  证书指纹不符（§2.2；此时没有发出任何升级请求，key 未泄露）
    6  网络不通 / 超时 / 连接被服务端直接断开（§2.3 末行：不要循环重连）
    7  音频文件格式不合格（--wav 必须 16k 单声道 16 位；不做重采样）
    8  协议异常（连接在 task-finished 之前被关闭、收到无法解析的消息等）
"""
import argparse
import base64
import hashlib
import ipaddress
import json
import os
import re
import secrets
import select
import socket
import ssl
import struct
import sys
import threading
import time
import uuid
import wave
from urllib.parse import unquote

# ---------------------------------------------------------------- 退出码
EXIT_OK = 0
EXIT_USAGE = 2
EXIT_TASK_FAILED = 3
EXIT_REJECTED = 4
EXIT_FP_MISMATCH = 5
EXIT_NETWORK = 6
EXIT_AUDIO = 7
EXIT_PROTOCOL = 8

DEFAULT_PORT = 7860                 # §1.1 缺端口补 7860
PAIR_PREFIX = "feiyin-asr://pair?"  # §1.1
HANDSHAKE_TIMEOUT = 10.0            # §2.3 TLS + 升级须在 10 秒内完成
TCP_PROBE_TIMEOUT = 5.0
WAN_GRACE = 0.3                     # §2.1 外网先通后再等内网 300ms
FRAME_MS = 100                      # §4.2 建议每帧 100ms
SAMPLE_RATE = 16000                 # §4.1 一期只收 16000
FRAME_BYTES = SAMPLE_RATE * 2 * FRAME_MS // 1000  # 3200 字节
MAX_MESSAGE = 1 << 20               # §3 单条消息上限 1MB
DEFAULT_DEVICE_NAME = "python-ref-client"
WS_GUID = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11"  # RFC 6455 §1.3
ID_FILE = os.path.join(os.path.dirname(os.path.abspath(__file__)), ".feiyin-client.json")


# ---------------------------------------------------------------- 异常
class ClientError(Exception):
    """带退出码的客户端错误；main() 统一转成退出码。"""
    exit_code = EXIT_PROTOCOL


class UsageError(ClientError):
    exit_code = EXIT_USAGE


class AudioFormatError(ClientError):
    exit_code = EXIT_AUDIO


class NetworkError(ClientError):
    exit_code = EXIT_NETWORK


class FingerprintMismatch(ClientError):
    exit_code = EXIT_FP_MISMATCH

    def __init__(self, expected, actual):
        super().__init__(f"证书指纹不符：配对时为 {expected}，实际为 {actual}。服务端证书变了，需要重新配对（§2.2）")
        self.expected, self.actual = expected, actual


class HandshakeRejected(ClientError):
    exit_code = EXIT_REJECTED

    def __init__(self, status, body):
        self.status, self.body = status, body
        super().__init__(f"升级握手被拒：HTTP {status}，错误码/正文：{body or '(空)'}")


class WebSocketClosed(ClientError):
    exit_code = EXIT_PROTOCOL

    def __init__(self, code=None, reason=""):
        self.code, self.reason = code, reason
        super().__init__(f"连接被对端关闭（code={code}, reason={reason!r}）")


# ================================================================ 配对 §1
class Pairing:
    """配对结果：lan/wan 已规整为 `主机:端口`（空串 = 没有），key 已规整，fp 为小写 64 位十六进制（可空）。"""

    def __init__(self, lan="", wan="", key="", fp=""):
        self.lan, self.wan, self.key, self.fp = lan, wan, key, fp

    def __repr__(self):  # 不打印 key
        return f"Pairing(lan={self.lan!r}, wan={self.wan!r}, key=<{len(self.key)}字符>, fp={self.fp!r})"


def normalize_key(key):
    """§1.2：去掉所有空白和 `-`，转大写。"""
    return re.sub(r"[\s\-]+", "", key or "").upper()


def normalize_fp(fp):
    """§1.1：64 位十六进制，统一小写。
    （网关实现还容忍冒号/空白分隔，见 asr_shared/remote.rs；文档未写，此处同样宽容。）"""
    fp = re.sub(r"[\s:]+", "", fp or "").lower()
    if not re.fullmatch(r"[0-9a-f]{64}", fp):
        raise UsageError("指纹必须是 64 位十六进制（证书 DER 的 SHA-256）")
    return fp


def normalize_endpoint(s):
    """§1.1：`主机:端口`；缺端口补 7860；IPv6 须带方括号。空串返回空串。"""
    s = (s or "").strip()
    if not s:
        return ""
    if re.search(r"\s", s) or "/" in s:
        raise UsageError(f"地址格式不对：{s!r}")
    port = None
    if s.startswith("["):
        host_end = s.find("]")
        if host_end < 0:
            raise UsageError(f"IPv6 地址缺少右方括号：{s!r}")
        host = s[: host_end + 1]
        tail = s[host_end + 1:]
        if tail.startswith(":"):
            port = tail[1:]
        elif tail:
            raise UsageError(f"地址格式不对：{s!r}")
    else:
        if s.count(":") >= 2:
            raise UsageError(f"IPv6 主机须带方括号，如 [fe80::1]:7860：{s!r}")
        host, _, p = s.partition(":")
        port = p if ":" in s else None
    if host in ("", "[]"):
        raise UsageError(f"地址缺少主机：{s!r}")
    if port is None:
        port_n = DEFAULT_PORT
    else:
        if not port.isdigit() or not (0 < int(port) <= 65535):
            raise UsageError(f"端口不合法：{s!r}")
        port_n = int(port)
    return f"{host}:{port_n}"


def split_endpoint(ep):
    """`主机:端口` -> (主机, 端口)；IPv6 去掉方括号。"""
    if ep.startswith("["):
        host, _, rest = ep[1:].partition("]")
        return host, int(rest[1:])
    host, _, port = ep.rpartition(":")
    return host, int(port)


def _finish_pairing(lan, wan, key, fp, *, need_key, need_fp):
    lan, wan = normalize_endpoint(lan), normalize_endpoint(wan)
    if not lan and not wan:
        raise UsageError("内网 / 外网地址至少要有一个（§1.1）")
    key = normalize_key(key)
    if need_key and not key:
        raise UsageError("缺少 key（§1.1）")
    fp = normalize_fp(fp) if (fp or "").strip() else ""
    if need_fp and not fp:
        raise UsageError("连接码缺少 fp（§1.1 fp 必有）")
    return Pairing(lan, wan, key, fp)


def parse_pair_uri(code):
    """§1.1：解析 `feiyin-asr://pair?v=1&lan=..&wan=..&key=..&fp=..`。
    百分号解码；未知字段忽略；v 必须是 1（否则拒绝并提示升级）；fp、key 必有；lan/wan 二选一。"""
    code = (code or "").strip()
    if not code.startswith(PAIR_PREFIX):
        raise UsageError("不是飞音连接码（应以 feiyin-asr://pair? 开头）")
    fields = {}
    for kv in code[len(PAIR_PREFIX):].split("&"):
        k, _, v = kv.partition("=")
        fields[k] = unquote(v)  # 先解码（§1.1）；后出现的同名字段覆盖先出现的，与网关一致
    if fields.get("v") != "1":
        raise UsageError(f"连接码版本 v={fields.get('v')!r} 不认识（当前只认 1），请升级客户端（§1.1）")
    return _finish_pairing(fields.get("lan", ""), fields.get("wan", ""), fields.get("key", ""),
                           fields.get("fp", ""), need_key=True, need_fp=True)


def pairing_from_args(args):
    """--code 与 --lan/--wan/--key/--fp 二选一。手输时指纹可空（§1.2）。"""
    manual = any([args.lan, args.wan, args.key, args.fp])
    if args.code and manual:
        raise UsageError("--code 与 --lan/--wan/--key/--fp 只能二选一")
    if args.code:
        return parse_pair_uri(args.code)
    if not manual:
        raise UsageError("需要 --code，或 --lan/--wan 配合 --key（--probe 时可不给 key）")
    # probe 只握手不发 key（§1.2），所以 probe 时 key 可省
    return _finish_pairing(args.lan or "", args.wan or "", args.key or "", args.fp or "",
                           need_key=not args.probe, need_fp=False)


# ================================================================ 本机身份 §1.4
def load_device_id(path=ID_FILE):
    """§1.4：首次随机生成 16 位字母数字，存本地，之后永久不变。"""
    try:
        with open(path, "r", encoding="utf-8") as f:
            did = str(json.load(f).get("device_id", "")).strip()
        if did:
            return did
    except (OSError, ValueError, AttributeError):
        pass
    alphabet = "abcdefghijklmnopqrstuvwxyz0123456789"
    did = "".join(secrets.choice(alphabet) for _ in range(16))
    try:
        with open(path, "w", encoding="utf-8") as f:
            json.dump({"device_id": did}, f)
    except OSError as e:
        print(f"[警告] 无法保存 device_id 到 {path}: {e}（下次会换新 id，key 会因 DeviceMismatch 失效）", file=sys.stderr)
    return did


# ================================================================ WebSocket (RFC 6455)
OP_CONT, OP_TEXT, OP_BIN, OP_CLOSE, OP_PING, OP_PONG = 0x0, 0x1, 0x2, 0x8, 0x9, 0xA


def apply_mask(data, key):
    """RFC 6455 §5.3：data[i] ^= key[i % 4]。用大整数异或，1MB 也很快。"""
    n = len(data)
    if n == 0:
        return b""
    k = (key * (n // 4 + 1))[:n]
    return (int.from_bytes(data, "big") ^ int.from_bytes(k, "big")).to_bytes(n, "big")


def encode_frame(opcode, payload=b"", *, mask=True, fin=True, mask_key=None):
    """编码一帧。客户端 -> 服务端的帧必须 mask=True（RFC 6455 §5.1）；服务端帧不掩码（测试假网关用）。
    长度：<126 一字节；<65536 用 126+2 字节；否则 127+8 字节。"""
    b0 = (0x80 if fin else 0) | opcode
    n = len(payload)
    mbit = 0x80 if mask else 0
    if n < 126:
        head = struct.pack("!BB", b0, mbit | n)
    elif n < 65536:
        head = struct.pack("!BBH", b0, mbit | 126, n)
    else:
        head = struct.pack("!BBQ", b0, mbit | 127, n)
    if mask:
        key = mask_key if mask_key is not None else os.urandom(4)
        return head + key + apply_mask(payload, key)
    return head + payload


def read_frame(read_exact):
    """从 read_exact(n)->bytes 读一帧，返回 (fin, opcode, payload)；自动处理掩码与 126/127 扩展长度。"""
    b0, b1 = read_exact(2)
    fin = bool(b0 & 0x80)
    if b0 & 0x70:
        raise ClientError("收到带保留位的 WebSocket 帧（未协商扩展）")
    opcode = b0 & 0x0F
    masked = bool(b1 & 0x80)
    n = b1 & 0x7F
    if n == 126:
        (n,) = struct.unpack("!H", read_exact(2))
    elif n == 127:
        (n,) = struct.unpack("!Q", read_exact(8))
    if opcode >= 0x8 and (n > 125 or not fin):
        raise ClientError("非法控制帧（过长或被分片）")
    if n > MAX_MESSAGE * 4:  # 防御：对端声称超大帧时不去分配
        raise ClientError(f"帧过大：{n} 字节")
    key = read_exact(4) if masked else None
    payload = read_exact(n) if n else b""
    if key:
        payload = apply_mask(payload, key)
    return fin, opcode, payload


def ws_accept_key(client_key):
    """RFC 6455 §4.2.2：base64(sha1(Sec-WebSocket-Key + GUID))。"""
    return base64.b64encode(hashlib.sha1((client_key + WS_GUID).encode("ascii")).digest()).decode("ascii")


class WebSocket:
    """已完成升级握手的连接。发送加锁（音频线程与主线程回 Pong 可能同时发）。"""

    def __init__(self, sock, leftover=b"", timeout=60.0):
        self.sock = sock
        self.timeout = timeout          # 连续这么久收不到任何字节 ⇒ socket.timeout
        self._buf = bytearray(leftover)
        self._io_lock = threading.Lock()
        self._closed = False
        # 音频线程发、主线程收，会并发使用同一个 SSLSocket；OpenSSL 的 TLS1.3 读写互相牵连，
        # 裸并发会偶发 WSAEWOULDBLOCK(10035)。所以：套接字置为非阻塞，每次真正的 send/recv 都在
        # 同一把锁里做，等待就绪放在锁外（select / 短睡眠）。
        sock.settimeout(0.0)

    def _wait_readable(self, deadline):
        pending = getattr(self.sock, "pending", None)
        if pending is not None and pending() > 0:
            return
        if time.monotonic() > deadline:
            raise socket.timeout("等待服务端数据超时")
        select.select([self.sock], [], [], 0.05)

    def _recv_some(self):
        deadline = time.monotonic() + self.timeout
        while True:
            self._wait_readable(deadline)
            with self._io_lock:
                try:
                    return self.sock.recv(65536)
                except (BlockingIOError, ssl.SSLWantReadError, ssl.SSLWantWriteError):
                    pass
            time.sleep(0.001)

    def _read_exact(self, n):
        while len(self._buf) < n:
            chunk = self._recv_some()
            if not chunk:
                raise WebSocketClosed(None, "TCP 连接被对端关闭")
            self._buf += chunk
        out = bytes(self._buf[:n])
        del self._buf[:n]
        return out

    def _send(self, opcode, payload=b""):
        view = memoryview(encode_frame(opcode, payload, mask=True))  # 客户端帧必须掩码
        deadline = time.monotonic() + max(self.timeout, 10.0)
        while view:
            with self._io_lock:
                try:
                    view = view[self.sock.send(view):]
                    continue
                except (BlockingIOError, ssl.SSLWantReadError, ssl.SSLWantWriteError):
                    pass
            if time.monotonic() > deadline:
                raise socket.timeout("发送超时")
            time.sleep(0.002)

    def send_text(self, text):
        self._send(OP_TEXT, text.encode("utf-8"))

    def send_binary(self, data):
        self._send(OP_BIN, data)

    def send_ping(self, data=b""):
        self._send(OP_PING, data)  # §3 空闲保活

    def close(self, code=1000):
        if not self._closed:
            self._closed = True
            try:
                self._send(OP_CLOSE, struct.pack("!H", code))  # §4.4 直接关连接即取消
            except OSError:
                pass
        try:
            self.sock.close()
        except OSError:
            pass

    def recv_message(self):
        """收一条完整消息，返回 (OP_TEXT|OP_BIN, bytes)。
        Ping 自动回 Pong；Pong 忽略；Close 回显后抛 WebSocketClosed；支持分片续帧。"""
        msg_op, parts, total = None, [], 0
        while True:
            fin, op, payload = read_frame(self._read_exact)
            if op == OP_PING:
                self._send(OP_PONG, payload)
                continue
            if op == OP_PONG:
                continue
            if op == OP_CLOSE:
                code = struct.unpack("!H", payload[:2])[0] if len(payload) >= 2 else None
                reason = payload[2:].decode("utf-8", "replace")
                if not self._closed:
                    self._closed = True
                    try:
                        self._send(OP_CLOSE, payload[:2])
                    except OSError:
                        pass
                raise WebSocketClosed(code, reason)
            if op in (OP_TEXT, OP_BIN):
                if msg_op is not None:
                    raise ClientError("上一条分片消息未结束又来了新消息")
                msg_op = op
            elif op == OP_CONT:
                if msg_op is None:
                    raise ClientError("收到没有起始帧的续帧")
            else:
                raise ClientError(f"未知 WebSocket 操作码 {op:#x}")
            parts.append(payload)
            total += len(payload)
            if total > MAX_MESSAGE:
                raise ClientError("服务端消息超过 1MB 上限")
            if fin:
                return msg_op, b"".join(parts)


def _read_http_head(sock):
    """读到 \\r\\n\\r\\n 为止，返回 (状态码, 头字典(小写键), 已多读的字节)。"""
    buf = b""
    while b"\r\n\r\n" not in buf:
        chunk = sock.recv(4096)
        if not chunk:
            raise NetworkError("升级握手时连接被服务端断开（可能被锁定，§2.3）")
        buf += chunk
        if len(buf) > 65536:
            raise ClientError("HTTP 响应头过长")
    head, _, rest = buf.partition(b"\r\n\r\n")
    lines = head.decode("latin-1").split("\r\n")
    m = re.match(r"HTTP/1\.[01] (\d{3})", lines[0])
    if not m:
        raise ClientError(f"不是合法的 HTTP 响应：{lines[0]!r}")
    headers = {}
    for ln in lines[1:]:
        k, _, v = ln.partition(":")
        headers[k.strip().lower()] = v.strip()
    return int(m.group(1)), headers, rest


def ws_upgrade(sock, host_header, key, path="/"):
    """§2.3：发升级请求（Authorization: Bearer <key>）并校验 101 与 Sec-WebSocket-Accept。
    非 101 抛 HandshakeRejected(status, 正文错误码)。调用方必须已通过指纹比对（§2.2）。"""
    ws_key = base64.b64encode(os.urandom(16)).decode("ascii")
    req = (f"GET {path} HTTP/1.1\r\nHost: {host_header}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n"
           f"Sec-WebSocket-Key: {ws_key}\r\nSec-WebSocket-Version: 13\r\n"
           f"Authorization: Bearer {key}\r\n\r\n")
    sock.sendall(req.encode("ascii"))
    status, headers, rest = _read_http_head(sock)
    if status != 101:
        # 401/503：正文是错误码（InvalidKey/KeyExpired/KeyRevoked/ServicePaused，§2.3）
        body = rest
        try:
            want = int(headers.get("content-length", "-1"))
            while want >= 0 and len(body) < want:
                chunk = sock.recv(4096)
                if not chunk:
                    break
                body += chunk
            if want < 0:  # 没有 Content-Length：读到对端关闭或超时
                while len(body) < 4096:
                    chunk = sock.recv(4096)
                    if not chunk:
                        break
                    body += chunk
        except OSError:
            pass
        raise HandshakeRejected(status, body.decode("utf-8", "replace").strip())
    if headers.get("sec-websocket-accept", "") != ws_accept_key(ws_key):
        raise ClientError("Sec-WebSocket-Accept 校验失败（对端不是合规的 WebSocket 服务端）")
    if headers.get("upgrade", "").lower() != "websocket":
        raise ClientError("响应缺少 Upgrade: websocket")
    return WebSocket(sock, rest)


# ================================================================ 连接 §2
def tcp_probe(endpoint, timeout=TCP_PROBE_TIMEOUT):
    """§2.1：只试 TCP 连接（不带 key，不做 TLS）。"""
    host, port = split_endpoint(endpoint)
    try:
        with socket.create_connection((host, port), timeout=timeout):
            return True
    except OSError:
        return False


def pick_endpoint(pairing, mode="auto", probe=tcp_probe, wan_grace=WAN_GRACE):
    """§2.1：选地址，返回 (label, endpoint)。
    lan/wan：直接用指定的；auto：两地址同时试 TCP，内网通了用内网；外网先通则再等内网 wan_grace 秒，
    内网仍不通才用外网。只有一个地址时不必探测，直接用。"""
    if mode == "lan":
        if not pairing.lan:
            raise UsageError("--mode lan 但没有内网地址")
        return "lan", pairing.lan
    if mode == "wan":
        if not pairing.wan:
            raise UsageError("--mode wan 但没有外网地址")
        return "wan", pairing.wan
    if pairing.lan and not pairing.wan:
        return "lan", pairing.lan
    if pairing.wan and not pairing.lan:
        return "wan", pairing.wan
    eps = {"lan": pairing.lan, "wan": pairing.wan}
    res = {}  # label -> True/False（缺省 = 还没出结果）

    def work(label):
        res[label] = bool(probe(eps[label]))

    for label in eps:
        threading.Thread(target=work, args=(label,), daemon=True).start()
    wan_ok_at = None
    while True:
        lan, wan = res.get("lan"), res.get("wan")
        if lan is True:
            return "lan", pairing.lan
        if lan is False:
            if wan is True:
                return "wan", pairing.wan
            if wan is False:
                raise NetworkError("内网、外网地址都连不上")
        elif wan is True:
            wan_ok_at = wan_ok_at or time.monotonic()
            if time.monotonic() - wan_ok_at >= wan_grace:
                return "wan", pairing.wan
        time.sleep(0.01)


def _tls_context():
    """§2.2：只允许 TLS 1.3；自签证书，不做 CA 校验、不校验主机名（改用钉扎指纹）。"""
    ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
    ctx.minimum_version = ssl.TLSVersion.TLSv1_3
    ctx.maximum_version = ssl.TLSVersion.TLSv1_3
    ctx.check_hostname = False
    ctx.verify_mode = ssl.CERT_NONE
    return ctx


def tls_connect(endpoint, timeout=HANDSHAKE_TIMEOUT):
    """TCP + TLS1.3 握手，返回 (ssl_socket, 证书指纹小写hex)。此时还没发任何应用数据。"""
    host, port = split_endpoint(endpoint)
    try:
        raw = socket.create_connection((host, port), timeout=timeout)
    except OSError as e:
        raise NetworkError(f"连接 {endpoint} 失败：{e}")
    try:
        try:
            ipaddress.ip_address(host)
            sni = None  # IP 不发 SNI
        except ValueError:
            sni = host
        tls = _tls_context().wrap_socket(raw, server_hostname=sni)
        tls.settimeout(timeout)
        der = tls.getpeercert(binary_form=True)
    except (ssl.SSLError, OSError) as e:
        raw.close()
        raise NetworkError(f"TLS 1.3 握手失败：{e}（服务端未开 TLS1.3，或同一 IP 连续 5 次鉴权失败被锁 10 分钟，§2.3）")
    return tls, hashlib.sha256(der).hexdigest()


def probe_fingerprint(endpoint, timeout=HANDSHAKE_TIMEOUT):
    """§1.2 / --probe：只握手取回证书指纹，不发 key。"""
    tls, fp = tls_connect(endpoint, timeout)
    tls.close()
    return fp


def connect_pinned(endpoint, key, expected_fp, timeout=HANDSHAKE_TIMEOUT):
    """TLS 握手 -> 钉扎指纹 -> 升级。返回 WebSocket。"""
    tls, actual = tls_connect(endpoint, timeout)
    # §2.2 顺序硬要求：指纹比对通过之后才能发 WebSocket 升级请求。
    # key 在升级请求头里，先发后验等于把 key 交给中间人；所以不符就关连接，升级请求一个字节都不发。
    if actual.lower() != expected_fp.lower():
        tls.close()
        raise FingerprintMismatch(expected_fp.lower(), actual)
    try:
        return ws_upgrade(tls, endpoint, key)
    except (HandshakeRejected, ClientError):
        tls.close()
        raise
    except OSError as e:
        tls.close()
        raise NetworkError(f"升级握手时网络错误：{e}")


# ================================================================ 消息 §4
def build_run_task(task_id, device_id, device_name, *, itn=True, punct=True, translate=False, vocab=None):
    """§4.1 run-task。字段名与文档一致；vocabulary 仅在给了热词时出现。"""
    params = {
        "format": "pcm",
        "sample_rate": SAMPLE_RATE,
        "inverse_text_normalization_enabled": bool(itn),
        "punctuation_prediction_enabled": bool(punct),
        "translation_enabled": bool(translate),
        "feiyin": {"device_id": device_id, "device_name": device_name},
    }
    if vocab:
        params["vocabulary"] = list(vocab)
    return {
        "header": {"action": "run-task", "task_id": task_id, "streaming": "duplex"},
        "payload": {"task_group": "audio", "task": "asr", "function": "recognition",
                    "model": "feiyin-local", "input": {}, "parameters": params},
    }


def build_finish_task(task_id):
    """§4.3 finish-task。"""
    return {"header": {"action": "finish-task", "task_id": task_id, "streaming": "duplex"},
            "payload": {"input": {}}}


# ================================================================ 音频 §4.2
def load_wav_pcm(path):
    """读 WAV，必须 16k / 单声道 / 16 位，否则 AudioFormatError（不做重采样）。返回裸 PCM。"""
    try:
        with wave.open(path, "rb") as w:
            ch, width, rate = w.getnchannels(), w.getsampwidth(), w.getframerate()
            if (ch, width, rate) != (1, 2, SAMPLE_RATE):
                raise AudioFormatError(
                    f"WAV 必须是 16kHz 单声道 16 位（§4.2），当前是 {rate}Hz / {ch} 声道 / {width * 8} 位；本客户端不重采样")
            return w.readframes(w.getnframes())
    except (wave.Error, EOFError) as e:
        raise AudioFormatError(f"不是可读的 PCM WAV 文件：{e}")
    except OSError as e:
        raise AudioFormatError(f"读不了音频文件：{e}")


def load_pcm(path):
    """裸 16k s16le。"""
    try:
        with open(path, "rb") as f:
            data = f.read()
    except OSError as e:
        raise AudioFormatError(f"读不了音频文件：{e}")
    if len(data) % 2:
        data = data[:-1]  # 16 位采样必须偶数字节
    return data


def iter_frames(pcm, frame_bytes=FRAME_BYTES):
    for i in range(0, len(pcm), frame_bytes):
        yield pcm[i:i + frame_bytes]


# ================================================================ 输出
class Printer:
    def __init__(self, as_json=False, out=None, err=None):
        self.as_json = as_json
        self.out = out or sys.stdout
        self.err = err or sys.stderr
        self._inline = False  # 终端上的预览行是否还没换行
        self._tty = (not as_json) and hasattr(self.out, "isatty") and self.out.isatty()

    def log(self, msg):
        """进度说明走 stderr，不污染 --json 的 stdout。"""
        print(msg, file=self.err)

    def event(self, ev):
        """--json：每个事件原样一行。"""
        print(json.dumps(ev, ensure_ascii=False), file=self.out, flush=True)

    def preview(self, text, tentative):
        # §5.2：每条预览整体替换上一条；末尾 tentative 个字还没稳定，用【】标出
        t = max(0, min(int(tentative or 0), len(text)))
        shown = text if t == 0 else f"{text[:-t]}【{text[-t:]}】"
        if self._tty:
            print(f"\r\x1b[2K预览: {shown}", end="", file=self.out, flush=True)
            self._inline = True
        else:
            print(f"[预览] {shown}", file=self.out, flush=True)

    def final(self, sentence, feiyin, translations, usage):
        if self._inline:
            print("", file=self.out)
            self._inline = False
        text = sentence.get("text", "")
        print(f"[终稿] {text if text else '(空：没听到有效语音)'}", file=self.out)
        words = sentence.get("words") or []
        srcs = (feiyin or {}).get("ts_source") or []
        if words:
            print("  逐词时间(ms)  词  ts_source", file=self.out)
            for i, w in enumerate(words):
                src = srcs[i] if i < len(srcs) else "?"
                print(f"  {w.get('begin_time')}-{w.get('end_time')}  "
                      f"{w.get('text', '')}{w.get('punctuation', '')}  {src}", file=self.out)
        for tr in translations or []:
            print(f"[译文 {tr.get('lang')}] {tr.get('text', '')}", file=self.out)
        if usage and "duration" in usage:
            print(f"音频时长：{usage['duration']} 秒", file=self.out)
        self.out.flush()

    def end_inline(self):
        if self._inline:
            print("", file=self.out)
            self._inline = False


# ================================================================ 识别流程 §6
def stream_audio(ws, pcm, task_id, fast, errors):
    """独立线程：按 100ms 一帧、实时节奏发音频（§4.2），发完发 finish-task（§4.3）。"""
    try:
        t_next = time.monotonic()
        for frame in iter_frames(pcm):
            ws.send_binary(frame)
            if not fast:
                t_next += len(frame) / (SAMPLE_RATE * 2)
                delay = t_next - time.monotonic()
                if delay > 0:
                    time.sleep(delay)
        ws.send_text(json.dumps(build_finish_task(task_id)))
    except Exception as e:  # 主线程会因连接断开同时报错，这里只留证据
        errors.append(e)


def run_recognition(ws, pcm, args, device_id, device_name, printer):
    """run-task -> 等 task-started -> 发音频 -> finish-task -> 收事件直到 task-finished/task-failed。返回退出码。"""
    task_id = uuid.uuid4().hex  # §4.1 每段新生成 32 位十六进制
    msg = build_run_task(task_id, device_id, device_name, itn=not args.no_itn, punct=not args.no_punct,
                         translate=args.translate, vocab=args.vocab_list)
    ws.send_text(json.dumps(msg))
    sender, errors = None, []
    while True:
        op, data = ws.recv_message()
        if op != OP_TEXT:
            continue  # 服务端不发二进制；忽略
        try:
            ev = json.loads(data.decode("utf-8"))
            name = ev["header"]["event"]
        except (ValueError, KeyError, TypeError):
            printer.log(f"[协议异常] 无法解析的消息：{data[:200]!r}")
            return EXIT_PROTOCOL
        if printer.as_json:
            printer.event(ev)
        if name == "task-started":  # §5.1 收到后才能发音频
            sender = threading.Thread(target=stream_audio, args=(ws, pcm, task_id, args.fast, errors), daemon=True)
            sender.start()
        elif name == "result-generated":  # §5.2
            out = ev.get("payload", {}).get("output", {})
            sentence, fy = out.get("sentence", {}), out.get("feiyin", {})
            if not printer.as_json:
                if sentence.get("sentence_end"):  # 终稿，只出现一次
                    printer.final(sentence, fy, out.get("translations"), None)
                else:  # 预览：text 整体替换，不拼接
                    printer.preview(sentence.get("text", ""), fy.get("tentative", 0))
        elif name == "task-finished":  # §5.4
            if not printer.as_json:
                usage = ev.get("payload", {}).get("usage") or {}
                printer.end_inline()
                if "duration" in usage:
                    print(f"音频时长：{usage['duration']} 秒", file=printer.out)
            if sender:
                sender.join(timeout=2)
            return EXIT_OK
        elif name == "task-failed":  # §5.4
            printer.end_inline()
            h = ev.get("header", {})
            code = h.get("error_code", "?")
            printer.log(f"[task-failed] {code}: {h.get('error_message', '')}")
            hint = {
                "InvalidKey": "需要重新配对", "KeyExpired": "需要重新配对", "KeyRevoked": "需要重新配对",
                "DeviceMismatch": "需要重新配对", "ServicePaused": "对方已暂停，稍后再试",
                "ServiceBusy": "对方正忙，稍后再试", "ModelNotReady": "对方识别模型未就绪",
                "InternalError": "识别失败，可重试",
            }.get(code)
            if hint:
                printer.log(f"提示：{hint}（§5.4）")
            return EXIT_TASK_FAILED
        # 其它事件忽略


# ================================================================ 命令行
def build_parser():
    p = argparse.ArgumentParser(description="飞音识别服务 Python 参考客户端（见 docs/ASR-PROTOCOL.md）")
    p.add_argument("--code", help="连接码 feiyin-asr://pair?...（§1.1）")
    p.add_argument("--lan", help="内网地址 主机[:端口]")
    p.add_argument("--wan", help="外网地址 主机[:端口]")
    p.add_argument("--key", help="终端 key（可带空白/连字符，大小写不限）")
    p.add_argument("--fp", help="服务端证书指纹（SHA-256 小写十六进制 64 位）")
    p.add_argument("--probe", action="store_true", help="只做 TLS 握手取回证书指纹并打印，不发 key（§1.2）")
    p.add_argument("--mode", choices=["auto", "lan", "wan"], default="auto", help="选地址方式（§2.1），默认 auto")
    p.add_argument("--device-id", help="本机 device_id（缺省读写脚本同目录 .feiyin-client.json）")
    p.add_argument("--device-name", default=DEFAULT_DEVICE_NAME, help=f"本机名称，默认 {DEFAULT_DEVICE_NAME}")
    g = p.add_mutually_exclusive_group()
    g.add_argument("--wav", help="16k 单声道 16 位 WAV 文件")
    g.add_argument("--pcm", help="裸 16k s16le PCM 文件")
    p.add_argument("--fast", action="store_true", help="不按实时节奏，尽快发完")
    p.add_argument("--no-itn", action="store_true", help="关闭数字规整（§4.1）")
    p.add_argument("--no-punct", action="store_true", help="关闭标点（§4.1）")
    p.add_argument("--translate", action="store_true", help="终稿附译文（§5.3）")
    p.add_argument("--vocab", default="", help="热词，逗号分隔（§4.1 vocabulary）")
    p.add_argument("--json", action="store_true", help="每个事件原样输出一行 JSON")
    p.add_argument("--timeout", type=float, default=60.0, help="等待服务端事件的超时秒数，默认 60")
    return p


def main(argv=None, out=None, err=None):
    args = build_parser().parse_args(argv)
    printer = Printer(args.json, out, err)
    try:
        return _run(args, printer)
    except ClientError as e:
        printer.log(f"[错误] {e}")
        return e.exit_code
    except (socket.timeout, TimeoutError):
        printer.log("[错误] 等待服务端超时")
        return EXIT_NETWORK
    except OSError as e:
        printer.log(f"[错误] 网络错误：{e}")
        return EXIT_NETWORK


def _run(args, printer):
    args.vocab_list = [w.strip() for w in args.vocab.split(",") if w.strip()]
    pairing = pairing_from_args(args)

    if not args.probe:
        if not (args.wav or args.pcm):
            raise UsageError("需要 --wav 或 --pcm（或用 --probe）")
        if not pairing.fp:
            # §1.2：指纹为空时先 --probe 取回、让用户核对；未经确认的指纹不得带 key 连接
            raise UsageError("没有指纹：请先用 --probe 取回并与服务端设置页核对，再用 --fp 传入（§1.2）")
        device_id = (args.device_id or "").strip() or load_device_id()
        device_name = (args.device_name or "").strip()
        if not device_id or not device_name:
            raise UsageError("device_id / device_name 不能为空（§1.4）")
        # 先读音频：格式不对就在连接之前退出，别白占一次连接
        pcm = load_wav_pcm(args.wav) if args.wav else load_pcm(args.pcm)

    label, endpoint = pick_endpoint(pairing, args.mode)  # §2.1
    printer.log(f"[连接] {label} {endpoint}")

    if args.probe:
        fp = probe_fingerprint(endpoint)
        print(fp, file=printer.out)
        if pairing.fp and pairing.fp != fp:
            raise FingerprintMismatch(pairing.fp, fp)
        return EXIT_OK

    ws = connect_pinned(endpoint, pairing.key, pairing.fp)  # §2.2 + §2.3
    ws.timeout = args.timeout
    printer.log("[连接] 已升级为 WebSocket")
    try:
        return run_recognition(ws, pcm, args, device_id, device_name, printer)
    except WebSocketClosed as e:
        printer.end_inline()
        printer.log(f"[错误] 在 task-finished 之前连接被关闭：{e}")
        return EXIT_PROTOCOL
    finally:
        ws.close()


if __name__ == "__main__":
    sys.exit(main())
