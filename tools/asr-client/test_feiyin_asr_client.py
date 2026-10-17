# -*- coding: utf-8 -*-
"""feiyin_asr_client 的单元测试 + 本地回环集成测试（不需要真实服务端）。

运行：py -3 -m unittest discover -s tools/asr-client -v
集成测试需要本机有 `openssl` 命令来生成自签证书（标准库不能生成证书）；没有则 skip。
"""
import base64
import contextlib
import hashlib
import io
import json
import os
import shutil
import socket
import ssl
import struct
import subprocess
import sys
import tempfile
import threading
import time
import unittest
import wave

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import feiyin_asr_client as c  # noqa: E402

FP = "3f9a" * 16          # 64 位十六进制
KEY = "ABCDEFGHIJKLMNOPQRSTUVWXYZ"  # 测试用假 key
URI = f"feiyin-asr://pair?v=1&lan=192.168.1.5:7860&wan=asr.example.com:7860&key={KEY}&fp={FP}"


# ------------------------------------------------------------ 连接码 §1.1 / §1.2
class PairingTests(unittest.TestCase):
    def test_normal(self):
        p = c.parse_pair_uri(URI)
        self.assertEqual((p.lan, p.wan, p.key, p.fp), ("192.168.1.5:7860", "asr.example.com:7860", KEY, FP))

    def test_version_2_rejected(self):
        with self.assertRaises(c.UsageError):
            c.parse_pair_uri(URI.replace("v=1", "v=2"))

    def test_version_missing_rejected(self):
        with self.assertRaises(c.UsageError):
            c.parse_pair_uri(URI.replace("v=1&", ""))

    def test_missing_fp(self):
        with self.assertRaises(c.UsageError):
            c.parse_pair_uri(f"feiyin-asr://pair?v=1&lan=1.2.3.4:7860&key={KEY}")

    def test_missing_key_and_address(self):
        with self.assertRaises(c.UsageError):
            c.parse_pair_uri(f"feiyin-asr://pair?v=1&lan=1.2.3.4&fp={FP}")
        with self.assertRaises(c.UsageError):
            c.parse_pair_uri(f"feiyin-asr://pair?v=1&key={KEY}&fp={FP}")

    def test_bad_prefix(self):
        with self.assertRaises(c.UsageError):
            c.parse_pair_uri("http://example.com/?v=1")

    def test_percent_encoded(self):
        code = f"feiyin-asr://pair?v=1&lan=192.168.1.5%3A7861&key=ABCD%2DEFGH&fp={FP.upper()}"
        p = c.parse_pair_uri(code)
        self.assertEqual(p.lan, "192.168.1.5:7861")
        self.assertEqual(p.key, "ABCDEFGH")  # 解码后再规整：去连字符
        self.assertEqual(p.fp, FP)           # 指纹统一小写

    def test_ipv6(self):
        code = f"feiyin-asr://pair?v=1&lan=%5Bfe80%3A%3A1%5D%3A9000&wan=[2001:db8::2]&key={KEY}&fp={FP}"
        p = c.parse_pair_uri(code)
        self.assertEqual(p.lan, "[fe80::1]:9000")
        self.assertEqual(p.wan, "[2001:db8::2]:7860")
        self.assertEqual(c.split_endpoint(p.lan), ("fe80::1", 9000))

    def test_default_port(self):
        p = c.parse_pair_uri(f"feiyin-asr://pair?v=1&lan=10.0.0.2&key={KEY}&fp={FP}")
        self.assertEqual(p.lan, "10.0.0.2:7860")
        self.assertEqual(p.wan, "")

    def test_unknown_fields_ignored(self):
        p = c.parse_pair_uri(URI + "&future=xyz&x=1")
        self.assertEqual(p.key, KEY)

    def test_bare_ipv6_rejected(self):
        with self.assertRaises(c.UsageError):
            c.normalize_endpoint("fe80::1")

    def test_bad_port(self):
        for bad in ("a.com:0", "a.com:99999", "a.com:x"):
            with self.assertRaises(c.UsageError):
                c.normalize_endpoint(bad)

    def test_key_normalize(self):
        self.assertEqual(c.normalize_key(" abcd-efgh ijkl\tmn\n"), "ABCDEFGHIJKLMN")

    def test_manual_without_key_for_probe(self):
        args = c.build_parser().parse_args(["--lan", "10.0.0.2", "--probe"])
        self.assertEqual(c.pairing_from_args(args).lan, "10.0.0.2:7860")
        args = c.build_parser().parse_args(["--lan", "10.0.0.2"])
        with self.assertRaises(c.UsageError):
            c.pairing_from_args(args)

    def test_code_and_manual_exclusive(self):
        args = c.build_parser().parse_args(["--code", URI, "--lan", "10.0.0.2"])
        with self.assertRaises(c.UsageError):
            c.pairing_from_args(args)


# ------------------------------------------------------------ 选地址 §2.1
class PickEndpointTests(unittest.TestCase):
    P = c.Pairing("1.1.1.1:7860", "2.2.2.2:7860", KEY, FP)

    @staticmethod
    def fake(delays):
        """delays: {endpoint: (睡眠秒, 结果)}"""
        def probe(ep):
            d, ok = delays[ep]
            time.sleep(d)
            return ok
        return probe

    def test_lan_fast(self):
        r = c.pick_endpoint(self.P, probe=self.fake({"1.1.1.1:7860": (0, True), "2.2.2.2:7860": (0, True)}))
        self.assertEqual(r[0], "lan")

    def test_wan_first_but_lan_within_grace(self):
        r = c.pick_endpoint(self.P, wan_grace=0.5,
                            probe=self.fake({"1.1.1.1:7860": (0.1, True), "2.2.2.2:7860": (0, True)}))
        self.assertEqual(r[0], "lan")

    def test_lan_too_slow_uses_wan(self):
        r = c.pick_endpoint(self.P, wan_grace=0.1,
                            probe=self.fake({"1.1.1.1:7860": (1.0, True), "2.2.2.2:7860": (0, True)}))
        self.assertEqual(r[0], "wan")

    def test_lan_down(self):
        r = c.pick_endpoint(self.P, probe=self.fake({"1.1.1.1:7860": (0, False), "2.2.2.2:7860": (0.05, True)}))
        self.assertEqual(r[0], "wan")

    def test_both_down(self):
        with self.assertRaises(c.NetworkError):
            c.pick_endpoint(self.P, probe=self.fake({"1.1.1.1:7860": (0, False), "2.2.2.2:7860": (0, False)}))

    def test_forced_modes_and_single(self):
        self.assertEqual(c.pick_endpoint(self.P, "wan")[0], "wan")
        with self.assertRaises(c.UsageError):
            c.pick_endpoint(c.Pairing("1.1.1.1:7860", "", KEY, FP), "wan")
        self.assertEqual(c.pick_endpoint(c.Pairing("", "2.2.2.2:7860", KEY, FP))[0], "wan")


# ------------------------------------------------------------ WebSocket 编解码
def reader(data):
    buf = io.BytesIO(data)

    def read_exact(n):
        b = buf.read(n)
        if len(b) != n:
            raise EOFError
        return b
    return read_exact


class FrameTests(unittest.TestCase):
    def test_accept_key_rfc_vector(self):
        # RFC 6455 §1.3 的真实向量是 ...TxaQ9kYGzzhZRbK+xOo=（任务书里写的 ...Vxk7cAAB8n3pCx0yZhU= 有误）
        self.assertEqual(c.ws_accept_key("dGhlIHNhbXBsZSBub25jZQ=="), "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=")

    def test_roundtrip_all_length_classes(self):
        for n in (0, 1, 125, 126, 127, 65535, 65536, 70000):
            payload = os.urandom(n)
            for mask in (True, False):
                raw = c.encode_frame(c.OP_BIN, payload, mask=mask)
                fin, op, got = c.read_frame(reader(raw))
                self.assertEqual((fin, op, got), (True, c.OP_BIN, payload), f"n={n} mask={mask}")

    def test_length_header_shapes(self):
        self.assertEqual(c.encode_frame(c.OP_TEXT, b"x" * 125, mask=False)[1], 125)
        f126 = c.encode_frame(c.OP_TEXT, b"x" * 126, mask=False)
        self.assertEqual((f126[1], struct.unpack("!H", f126[2:4])[0]), (126, 126))
        f127 = c.encode_frame(c.OP_TEXT, b"x" * 65536, mask=False)
        self.assertEqual((f127[1], struct.unpack("!Q", f127[2:10])[0]), (127, 65536))

    def test_client_frame_is_masked(self):
        key = b"\x01\x02\x03\x04"
        raw = c.encode_frame(c.OP_TEXT, b"hello", mask=True, mask_key=key)
        self.assertTrue(raw[1] & 0x80)
        self.assertEqual(raw[2:6], key)
        self.assertNotEqual(raw[6:], b"hello")  # 载荷不是明文
        self.assertEqual(bytes(b ^ key[i % 4] for i, b in enumerate(raw[6:])), b"hello")

    def test_rfc_masked_example(self):
        # RFC 6455 §5.7：掩码后的 "Hello"
        raw = bytes([0x81, 0x85, 0x37, 0xfa, 0x21, 0x3d, 0x7f, 0x9f, 0x4d, 0x51, 0x58])
        self.assertEqual(c.read_frame(reader(raw)), (True, c.OP_TEXT, b"Hello"))
        self.assertEqual(c.encode_frame(c.OP_TEXT, b"Hello", mask=True, mask_key=bytes([0x37, 0xfa, 0x21, 0x3d])), raw)

    def test_fragmentation_ping_and_close(self):
        """用 socketpair 喂：分片 + 中间插 Ping + Close；验证 Pong 被回、消息被拼好、Close 被回显。"""
        a, b = socket.socketpair()
        ws = c.WebSocket(a)
        b.sendall(c.encode_frame(c.OP_TEXT, "你".encode(), mask=False, fin=False))
        b.sendall(c.encode_frame(c.OP_PING, b"pp", mask=False))
        b.sendall(c.encode_frame(c.OP_CONT, "好".encode(), mask=False, fin=True))
        b.sendall(c.encode_frame(c.OP_BIN, b"\x00\x01", mask=False))
        b.sendall(c.encode_frame(c.OP_CLOSE, struct.pack("!H", 1001) + "bye".encode(), mask=False))
        self.assertEqual(ws.recv_message(), (c.OP_TEXT, "你好".encode()))
        self.assertEqual(ws.recv_message(), (c.OP_BIN, b"\x00\x01"))
        with self.assertRaises(c.WebSocketClosed) as cm:
            ws.recv_message()
        self.assertEqual((cm.exception.code, cm.exception.reason), (1001, "bye"))
        # 对端应收到：Pong(payload=pp，带掩码)、Close 回显
        b.settimeout(2)
        rd = reader_from_sock(b)
        fin, op, pl = c.read_frame(rd)
        self.assertEqual((op, pl), (c.OP_PONG, b"pp"))
        fin, op, pl = c.read_frame(rd)
        self.assertEqual(op, c.OP_CLOSE)
        a.close(); b.close()

    def test_reserved_bits_rejected(self):
        with self.assertRaises(c.ClientError):
            c.read_frame(reader(bytes([0xC1, 0x00])))


def reader_from_sock(sock):
    def read_exact(n):
        out = b""
        while len(out) < n:
            chunk = sock.recv(n - len(out))
            if not chunk:
                raise EOFError
            out += chunk
        return out
    return read_exact


# ------------------------------------------------------------ 消息 §4
class MessageTests(unittest.TestCase):
    def test_run_task_shape(self):
        m = c.build_run_task("t1", "dev0123456789abc", "我的手机", itn=False, punct=False, translate=True,
                             vocab=["飞音", "LiveKit"])
        self.assertEqual(m["header"], {"action": "run-task", "task_id": "t1", "streaming": "duplex"})
        pl = m["payload"]
        for k, v in (("task_group", "audio"), ("task", "asr"), ("function", "recognition"),
                     ("model", "feiyin-local"), ("input", {})):
            self.assertEqual(pl[k], v)
        p = pl["parameters"]
        self.assertEqual(p["format"], "pcm")
        self.assertEqual(p["sample_rate"], 16000)
        self.assertIs(p["inverse_text_normalization_enabled"], False)
        self.assertIs(p["punctuation_prediction_enabled"], False)
        self.assertIs(p["translation_enabled"], True)
        self.assertEqual(p["vocabulary"], ["飞音", "LiveKit"])
        self.assertEqual(p["feiyin"], {"device_id": "dev0123456789abc", "device_name": "我的手机"})

    def test_run_task_defaults(self):
        p = c.build_run_task("t", "d", "n")["payload"]["parameters"]
        self.assertIs(p["inverse_text_normalization_enabled"], True)
        self.assertIs(p["punctuation_prediction_enabled"], True)
        self.assertIs(p["translation_enabled"], False)
        self.assertNotIn("vocabulary", p)

    def test_finish_task_shape(self):
        self.assertEqual(c.build_finish_task("t1"),
                         {"header": {"action": "finish-task", "task_id": "t1", "streaming": "duplex"},
                          "payload": {"input": {}}})

    def test_device_id_persisted(self):
        with tempfile.TemporaryDirectory() as d:
            path = os.path.join(d, ".feiyin-client.json")
            a = c.load_device_id(path)
            self.assertRegex(a, r"^[a-z0-9]{16}$")
            self.assertEqual(c.load_device_id(path), a)


# ------------------------------------------------------------ 音频 §4.2
def make_wav(path, rate=16000, ch=1, width=2, frames=1600):
    with wave.open(path, "wb") as w:
        w.setnchannels(ch); w.setsampwidth(width); w.setframerate(rate)
        w.writeframes(b"\x00" * (frames * ch * width))


class WavTests(unittest.TestCase):
    def setUp(self):
        self.d = tempfile.mkdtemp()
        self.addCleanup(shutil.rmtree, self.d, True)

    def test_ok(self):
        p = os.path.join(self.d, "a.wav")
        make_wav(p, frames=1600)
        self.assertEqual(len(c.load_wav_pcm(p)), 3200)

    def test_bad_formats(self):
        for kw in ({"rate": 44100}, {"ch": 2}, {"width": 1}):
            p = os.path.join(self.d, "b.wav")
            make_wav(p, **kw)
            with self.assertRaises(c.AudioFormatError, msg=str(kw)):
                c.load_wav_pcm(p)

    def test_not_wav_and_missing(self):
        p = os.path.join(self.d, "c.wav")
        with open(p, "wb") as f:
            f.write(b"not a wav")
        with self.assertRaises(c.AudioFormatError):
            c.load_wav_pcm(p)
        with self.assertRaises(c.AudioFormatError):
            c.load_wav_pcm(os.path.join(self.d, "nope.wav"))

    def test_frames_100ms(self):
        fr = list(c.iter_frames(b"\x00" * 7000))
        self.assertEqual([len(x) for x in fr], [3200, 3200, 600])

    def test_main_bad_wav_exit_code(self):
        p = os.path.join(self.d, "d.wav")
        make_wav(p, rate=8000)
        rc = c.main(["--code", URI, "--wav", p, "--device-id", "x"], io.StringIO(), io.StringIO())
        self.assertEqual(rc, c.EXIT_AUDIO)


# ------------------------------------------------------------ 本地回环集成测试
def _make_cert(tmp):
    exe = shutil.which("openssl")
    if not exe:
        return None
    cert, keyf = os.path.join(tmp, "c.pem"), os.path.join(tmp, "k.pem")
    r = subprocess.run([exe, "req", "-x509", "-newkey", "ec", "-pkeyopt", "ec_paramgen_curve:prime256v1",
                        "-nodes", "-keyout", keyf, "-out", cert, "-days", "2", "-subj", "/CN=localhost"],
                       capture_output=True)
    if r.returncode != 0:
        return None
    return cert, keyf


class FakeGateway(threading.Thread):
    """最小假网关：TLS1.3 + WebSocket 升级 + 按 §5 回事件。scenario: ok / reject401 / fail。"""

    def __init__(self, cert, keyf, key, scenario="ok"):
        super().__init__(daemon=True)
        ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        ctx.minimum_version = ssl.TLSVersion.TLSv1_3
        ctx.load_cert_chain(cert, keyf)
        self.ctx, self.key, self.scenario = ctx, key, scenario
        self.srv = socket.socket()
        self.srv.bind(("127.0.0.1", 0))
        self.srv.listen(4)
        self.port = self.srv.getsockname()[1]
        self.bytes_after_tls = 0   # TLS 握手后收到的应用层字节数（指纹不符时应为 0）
        self.auth_header = None
        self.run_task = None
        self.audio_bytes = 0
        self.got_finish = False
        self.request_seen = False
        with open(cert, "rb") as f:
            der = ssl.PEM_cert_to_DER_cert(f.read().decode())
        self.fp = hashlib.sha256(der).hexdigest()

    def run(self):
        self.srv.settimeout(15)
        while True:
            try:
                raw, _ = self.srv.accept()
            except OSError:
                return
            try:
                self.handle(raw)
            except Exception:
                pass
            finally:
                try:
                    raw.close()
                except OSError:
                    pass

    def handle(self, raw):
        raw.settimeout(5)
        tls = self.ctx.wrap_socket(raw, server_side=True)
        try:
            self._serve(tls)
        finally:
            tls.close()

    def _serve(self, tls):
        # 读升级请求头
        buf = b""
        while b"\r\n\r\n" not in buf:
            chunk = tls.recv(4096)
            if not chunk:
                return
            self.bytes_after_tls += len(chunk)
            buf += chunk
        self.request_seen = True
        lines = buf.decode().split("\r\n")
        h = {ln.split(":", 1)[0].strip().lower(): ln.split(":", 1)[1].strip() for ln in lines[1:] if ":" in ln}
        self.auth_header = h.get("authorization")
        if self.scenario == "reject401" or self.auth_header != f"Bearer {self.key}":
            body = b"InvalidKey"
            tls.sendall(b"HTTP/1.1 401 Unauthorized\r\nContent-Length: %d\r\n\r\n" % len(body) + body)
            return
        tls.sendall(("HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n"
                     f"Sec-WebSocket-Accept: {c.ws_accept_key(h['sec-websocket-key'])}\r\n\r\n").encode())
        rd = reader_from_sock(tls)
        send = lambda o: tls.sendall(c.encode_frame(c.OP_TEXT, json.dumps(o, ensure_ascii=False).encode(), mask=False))
        tid = None
        while True:
            fin, op, pl = c.read_frame(rd)
            assert pl is not None
            # 客户端帧必须掩码：第二字节最高位——read_frame 已解码，这里靠 encode 侧测试保证；
            if op == c.OP_CLOSE:
                return
            if op == c.OP_BIN:
                self.audio_bytes += len(pl)
                continue
            if op != c.OP_TEXT:
                continue
            msg = json.loads(pl)
            act = msg["header"]["action"]
            if act == "run-task":
                self.run_task, tid = msg, msg["header"]["task_id"]
                if self.scenario == "fail":
                    send({"header": {"task_id": tid, "event": "task-failed", "error_code": "ServiceBusy",
                                     "error_message": "capacity", "attributes": {}}, "payload": {}})
                    continue
                send({"header": {"task_id": tid, "event": "task-started", "attributes": {}}, "payload": {}})
            elif act == "finish-task":
                self.got_finish = True
                hd = {"task_id": tid, "event": "result-generated", "attributes": {}}
                send({"header": hd, "payload": {"output": {
                    "sentence": {"sentence_id": 1, "begin_time": 0, "end_time": None, "text": "今天买了三点五",
                                 "sentence_end": False, "words": []},
                    "feiyin": {"tentative": 2, "ts_source": []}}, "usage": None}})
                words = [{"begin_time": 0, "end_time": 300, "text": "今", "punctuation": ""},
                         {"begin_time": 300, "end_time": 600, "text": "天", "punctuation": "。"}]
                send({"header": hd, "payload": {"output": {
                    "sentence": {"sentence_id": 1, "begin_time": 0, "end_time": 600, "text": "今天。",
                                 "sentence_end": True, "words": words},
                    "feiyin": {"tentative": 0, "ts_source": ["aligned", "rough"]},
                    "translations": [{"sentence_id": 1, "lang": "en", "text": "Today.", "sentence_end": True}]},
                    "usage": None}})
                send({"header": {"task_id": tid, "event": "task-finished", "attributes": {}},
                      "payload": {"output": {}, "usage": {"duration": 1}}})


@unittest.skipUnless(shutil.which("openssl"), "本机没有 openssl 命令，无法生成自签证书，跳过回环集成测试")
class LoopbackTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.mkdtemp()
        made = _make_cert(cls.tmp)
        if not made:
            shutil.rmtree(cls.tmp, True)
            raise unittest.SkipTest("openssl 生成自签证书失败，跳过回环集成测试")
        cls.cert, cls.keyf = made
        cls.pcm = os.path.join(cls.tmp, "a.pcm")
        with open(cls.pcm, "wb") as f:
            f.write(b"\x00\x01" * 8000)  # 16000 字节 = 0.5 秒 = 5 帧

    @classmethod
    def tearDownClass(cls):
        shutil.rmtree(cls.tmp, True)

    def start(self, scenario="ok"):
        gw = FakeGateway(self.cert, self.keyf, KEY, scenario)
        gw.start()
        self.addCleanup(gw.srv.close)
        return gw

    def run_client(self, gw, *extra, fp=None, key=KEY):
        out, err = io.StringIO(), io.StringIO()
        argv = ["--lan", f"127.0.0.1:{gw.port}", "--key", key.lower(), "--fp", fp or gw.fp,
                "--pcm", self.pcm, "--fast", "--device-id", "dev0123456789abc", "--device-name", "测试机",
                "--timeout", "10", *extra]
        rc = c.main(argv, out, err)
        return rc, out.getvalue(), err.getvalue()

    def test_full_flow(self):
        gw = self.start()
        rc, out, err = self.run_client(gw, "--translate", "--vocab", "飞音, LiveKit", "--no-itn")
        self.assertEqual(rc, 0, err)
        self.assertEqual(gw.auth_header, f"Bearer {KEY}")  # key 规整：小写输入被转大写
        self.assertEqual(gw.audio_bytes, 16000)
        self.assertTrue(gw.got_finish)
        p = gw.run_task["payload"]["parameters"]
        self.assertEqual(p["vocabulary"], ["飞音", "LiveKit"])
        self.assertIs(p["translation_enabled"], True)
        self.assertIs(p["inverse_text_normalization_enabled"], False)
        self.assertEqual(p["feiyin"], {"device_id": "dev0123456789abc", "device_name": "测试机"})
        self.assertRegex(gw.run_task["header"]["task_id"], r"^[0-9a-f]{32}$")
        self.assertIn("[预览] 今天买了三【点五】", out)  # tentative=2 ⇒ 末 2 字标出
        self.assertIn("[终稿] 今天。", out)
        self.assertIn("0-300  今  aligned", out)
        self.assertIn("300-600  天。  rough", out)
        self.assertIn("[译文 en] Today.", out)

    def test_json_mode(self):
        gw = self.start()
        rc, out, err = self.run_client(gw, "--json")
        self.assertEqual(rc, 0, err)
        events = [json.loads(ln) for ln in out.strip().splitlines()]
        self.assertEqual([e["header"]["event"] for e in events],
                         ["task-started", "result-generated", "result-generated", "task-finished"])

    def test_fingerprint_mismatch_sends_no_upgrade(self):
        gw = self.start()
        rc, out, err = self.run_client(gw, fp="0" * 64)
        self.assertEqual(rc, c.EXIT_FP_MISMATCH, err)
        time.sleep(0.3)
        self.assertEqual(gw.bytes_after_tls, 0)   # §2.2：服务端 TLS 之后一个字节都没收到
        self.assertFalse(gw.request_seen)
        self.assertIsNone(gw.auth_header)         # key 没有离开本机

    def test_wrong_key_401(self):
        gw = self.start()
        rc, out, err = self.run_client(gw, key="WRONGKEYWRONGKEY")
        self.assertEqual(rc, c.EXIT_REJECTED)
        self.assertIn("401", err)
        self.assertIn("InvalidKey", err)

    def test_task_failed(self):
        gw = self.start("fail")
        rc, out, err = self.run_client(gw)
        self.assertEqual(rc, c.EXIT_TASK_FAILED)
        self.assertIn("ServiceBusy", err)

    def test_probe_returns_fingerprint_without_key(self):
        gw = self.start()
        out, err = io.StringIO(), io.StringIO()
        rc = c.main(["--lan", f"127.0.0.1:{gw.port}", "--probe"], out, err)
        self.assertEqual(rc, 0, err.getvalue())
        self.assertEqual(out.getvalue().strip(), gw.fp)
        time.sleep(0.3)
        self.assertFalse(gw.request_seen)  # probe 不发升级请求，更不发 key

    def test_no_fp_without_probe_refused(self):
        out, err = io.StringIO(), io.StringIO()
        rc = c.main(["--lan", "127.0.0.1:1", "--key", KEY, "--pcm", self.pcm], out, err)
        self.assertEqual(rc, c.EXIT_USAGE)

    def test_tls12_only_server_refused(self):
        """服务端只开 TLS1.2 ⇒ 客户端握手失败（§2.2 只用 TLS 1.3），不会降级。"""
        ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        ctx.maximum_version = ssl.TLSVersion.TLSv1_2
        ctx.load_cert_chain(self.cert, self.keyf)
        srv = socket.socket(); srv.bind(("127.0.0.1", 0)); srv.listen(1)
        self.addCleanup(srv.close)

        def acc():
            try:
                r, _ = srv.accept(); ctx.wrap_socket(r, server_side=True)
            except Exception:
                pass
        threading.Thread(target=acc, daemon=True).start()
        with self.assertRaises(c.NetworkError):
            c.tls_connect(f"127.0.0.1:{srv.getsockname()[1]}", timeout=3)


if __name__ == "__main__":
    unittest.main()
