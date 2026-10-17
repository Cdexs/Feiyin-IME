//! ASR-SERVER-491 S3：内核 ↔ 网关 IPC 传输（仅本机、仅当前用户；设计 §3）。
//!
//! - Windows：两条单向命名管道 `\\.\pipe\FeiyinIME.AsrCore.<pid>.in` / `.out`。用两条而不是一条双工：
//!   同步句柄上一个阻塞中的读会挡住另一线程的写（同一文件对象的同步 I/O 串行），单向两条最简单可靠。
//!   安全：DACL 只给当前用户；`PIPE_REJECT_REMOTE_CLIENTS` 拒网络客户端；`FILE_FLAG_FIRST_PIPE_INSTANCE`
//!   防抢注；名字带进程号（多用户同机互不冲突）。
//! - macOS：`$TMPDIR`（按用户隔离、0700）下的 Unix socket，文件权限 0600。
//! - 两端都要求首帧 `Hello` 带内核下发的口令，否则断开（防本机其他程序冒充网关）。

use std::io::{self, Read, Write};

use super::frame::{read_frame, Kind};

/// 一条已握手的连接（读端只给 IPC 读线程，写端由各会话线程经锁共用）。
pub(crate) struct Conn {
    pub reader: Box<dyn Read + Send>,
    pub writer: Box<dyn Write + Send>,
}

/// 校验首帧口令；不对 ⇒ `PermissionDenied`（调用方断开重等）。
fn handshake(reader: &mut dyn Read, token: &str) -> io::Result<()> {
    let mut r = reader;
    match read_frame(&mut r)? {
        Some(f) if f.kind == Kind::Hello && f.body == token.as_bytes() => Ok(()),
        _ => Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "asr gateway handshake failed",
        )),
    }
}

#[cfg(target_os = "windows")]
#[allow(unused_imports)] // CtlListener 只有网关用（网关以 #[path] 引入本文件）
pub(crate) use win::{CtlListener, Listener};

#[cfg(target_os = "windows")]
mod win {
    use super::*;
    use std::fs::File;
    use std::os::windows::io::FromRawHandle;
    use windows::core::{HSTRING, PCWSTR, PWSTR};
    use windows::Win32::Foundation::{
        CloseHandle, LocalFree, ERROR_PIPE_CONNECTED, HANDLE, HLOCAL, INVALID_HANDLE_VALUE,
    };
    use windows::Win32::Security::Authorization::{
        ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
        SDDL_REVISION_1,
    };
    use windows::Win32::Security::{
        GetTokenInformation, TokenUser, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES, TOKEN_QUERY,
        TOKEN_USER,
    };
    use windows::Win32::Storage::FileSystem::{
        FILE_FLAGS_AND_ATTRIBUTES, FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_DUPLEX,
        PIPE_ACCESS_INBOUND, PIPE_ACCESS_OUTBOUND,
    };
    use windows::Win32::System::Pipes::{
        ConnectNamedPipe, CreateNamedPipeW, PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS,
        PIPE_TYPE_BYTE, PIPE_WAIT,
    };
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    const PIPE_BUF: u32 = 64 * 1024;

    pub(crate) struct Listener {
        base: String,
        token: String,
    }

    impl Listener {
        pub(crate) fn bind(token: String) -> io::Result<Self> {
            Ok(Self {
                base: format!(r"\\.\pipe\FeiyinIME.AsrCore.{}", std::process::id()),
                token,
            })
        }

        /// 网关连接用的管道基名（网关在其后加 `.in` / `.out`）。
        pub(crate) fn endpoint(&self) -> &str {
            &self.base
        }

        /// 等一个网关连上并握手。两条管道都建好后先等 `.in`（网关先连它、发 Hello），再等 `.out`。
        pub(crate) fn accept(&self) -> io::Result<Conn> {
            let sa = OwnerOnly::new()?;
            let inbound = create_pipe(&format!("{}.in", self.base), PIPE_ACCESS_INBOUND, &sa)?;
            let outbound = create_pipe(&format!("{}.out", self.base), PIPE_ACCESS_OUTBOUND, &sa)?;
            wait_client(&inbound)?;
            let mut reader = inbound;
            handshake(&mut reader, &self.token)?;
            wait_client(&outbound)?;
            Ok(Conn {
                reader: Box::new(reader),
                writer: Box::new(outbound),
            })
        }

        /// 叫醒阻塞在 `accept` 的线程（停服务时用）：自己连一次 `.in`（在等它 ⇒ 握手失败返回），
        /// 再连一次 `.out`（网关连了 `.in` 后、连 `.out` 前被结束 ⇒ 卡在等 `.out`，靠这一下返回）。
        pub(crate) fn wake(&self) {
            let _ = std::fs::OpenOptions::new()
                .write(true)
                .open(format!("{}.in", self.base));
            let _ = std::fs::OpenOptions::new()
                .read(true)
                .open(format!("{}.out", self.base));
        }
    }

    /// 本机控制通道（S4b，网关 ↔ 设置界面）：双工管道，一次连接一问一答。安全措施同上。
    pub(crate) struct CtlListener {
        name: String,
    }

    impl CtlListener {
        pub(crate) fn bind(tag: &str) -> io::Result<Self> {
            Ok(Self {
                name: format!(r"\\.\pipe\FeiyinIME.{tag}.{}", std::process::id()),
            })
        }

        pub(crate) fn endpoint(&self) -> &str {
            &self.name
        }

        /// 建一个管道实例并等客户端连上（同一时刻只一个实例，客户端忙时重试）。
        pub(crate) fn accept(&self) -> io::Result<File> {
            let sa = OwnerOnly::new()?;
            let p = create_pipe(&self.name, PIPE_ACCESS_DUPLEX, &sa)?;
            wait_client(&p)?;
            Ok(p)
        }
    }

    fn create_pipe(
        name: &str,
        access: FILE_FLAGS_AND_ATTRIBUTES,
        sa: &OwnerOnly,
    ) -> io::Result<File> {
        let name = HSTRING::from(name);
        // SAFETY：名字与安全属性在调用期间有效；返回句柄交给 File 独占（drop 时关闭）。
        let h = unsafe {
            CreateNamedPipeW(
                PCWSTR(name.as_ptr()),
                access | FILE_FLAG_FIRST_PIPE_INSTANCE,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                1,
                PIPE_BUF,
                PIPE_BUF,
                0,
                Some(&sa.attrs),
            )
        };
        if h == INVALID_HANDLE_VALUE || h.is_invalid() {
            return Err(io::Error::last_os_error());
        }
        // SAFETY：h 为刚创建的有效句柄，所有权移交 File。
        Ok(unsafe { File::from_raw_handle(h.0 as _) })
    }

    fn wait_client(pipe: &File) -> io::Result<()> {
        use std::os::windows::io::AsRawHandle;
        // SAFETY：句柄由 File 持有且有效；同步模式（无 OVERLAPPED）阻塞到客户端连上。
        match unsafe { ConnectNamedPipe(HANDLE(pipe.as_raw_handle() as _), None) } {
            Ok(()) => Ok(()),
            Err(e) if e.code() == ERROR_PIPE_CONNECTED.to_hresult() => Ok(()),
            Err(e) => Err(io::Error::from_raw_os_error(e.code().0 & 0xFFFF)),
        }
    }

    /// 只授予当前用户的安全描述符（SDDL `D:P(A;;GA;;;<当前用户 SID>)`）。
    struct OwnerOnly {
        attrs: SECURITY_ATTRIBUTES,
        sd: PSECURITY_DESCRIPTOR,
    }

    impl OwnerOnly {
        fn new() -> io::Result<Self> {
            let sid = current_user_sid()?;
            let sddl = HSTRING::from(format!("D:P(A;;GA;;;{sid})"));
            let mut sd = PSECURITY_DESCRIPTOR::default();
            // SAFETY：输出指针可写；成功后 sd 由 LocalFree 释放（见 Drop）。
            unsafe {
                ConvertStringSecurityDescriptorToSecurityDescriptorW(
                    PCWSTR(sddl.as_ptr()),
                    SDDL_REVISION_1,
                    &mut sd,
                    None,
                )
            }
            .map_err(|e| io::Error::other(e.to_string()))?;
            Ok(Self {
                attrs: SECURITY_ATTRIBUTES {
                    nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
                    lpSecurityDescriptor: sd.0,
                    bInheritHandle: false.into(),
                },
                sd,
            })
        }
    }

    impl Drop for OwnerOnly {
        fn drop(&mut self) {
            // SAFETY：sd 由 ConvertStringSecurityDescriptorToSecurityDescriptorW 分配，只释放一次。
            unsafe {
                let _ = LocalFree(HLOCAL(self.sd.0));
            }
        }
    }

    fn current_user_sid() -> io::Result<String> {
        let mut token = HANDLE::default();
        // SAFETY：伪句柄 GetCurrentProcess 无需关闭；token 用完 CloseHandle。
        unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) }
            .map_err(|e| io::Error::other(e.to_string()))?;
        let mut len = 0u32;
        // SAFETY：第一次取所需长度（预期失败），第二次写入足长缓冲。
        unsafe {
            let _ = GetTokenInformation(token, TokenUser, None, 0, &mut len);
        }
        let mut buf = vec![0u8; len as usize];
        let r = unsafe {
            GetTokenInformation(
                token,
                TokenUser,
                Some(buf.as_mut_ptr() as *mut _),
                len,
                &mut len,
            )
        };
        // SAFETY：token 为有效句柄。
        unsafe {
            let _ = CloseHandle(token);
        }
        r.map_err(|e| io::Error::other(e.to_string()))?;
        // SAFETY：buf 为 GetTokenInformation(TokenUser) 写入的 TOKEN_USER。
        let user = unsafe { &*(buf.as_ptr() as *const TOKEN_USER) };
        let mut s = PWSTR::null();
        // SAFETY：SID 指针来自上面的缓冲（仍存活）；输出字符串由 LocalFree 释放。
        unsafe { ConvertSidToStringSidW(user.User.Sid, &mut s) }
            .map_err(|e| io::Error::other(e.to_string()))?;
        // SAFETY：s 为系统分配的以 NUL 结尾宽字符串。
        let out = unsafe { s.to_string() }.map_err(|e| io::Error::other(e.to_string()));
        unsafe {
            let _ = LocalFree(HLOCAL(s.0 as _));
        }
        out
    }
}

#[cfg(unix)]
#[allow(unused_imports)] // CtlListener 只有网关用（网关以 #[path] 引入本文件）
pub(crate) use unix::{CtlListener, Listener};

#[cfg(unix)]
mod unix {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::path::PathBuf;

    pub(crate) struct Listener {
        path: PathBuf,
        inner: UnixListener,
        token: String,
    }

    impl Listener {
        pub(crate) fn bind(token: String) -> io::Result<Self> {
            // macOS 的 TMPDIR 按用户隔离（0700）；文件名带进程号，同机多用户 / 多进程互不冲突。
            let path = std::env::temp_dir().join(format!("feiyin-asr-{}.sock", std::process::id()));
            let _ = std::fs::remove_file(&path);
            let inner = UnixListener::bind(&path)?;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
            Ok(Self { path, inner, token })
        }

        pub(crate) fn endpoint(&self) -> &str {
            self.path.to_str().unwrap_or_default()
        }

        pub(crate) fn accept(&self) -> io::Result<Conn> {
            let (stream, _) = self.inner.accept()?;
            let mut reader = stream.try_clone()?;
            handshake(&mut reader, &self.token)?;
            Ok(Conn {
                reader: Box::new(reader),
                writer: Box::new(stream),
            })
        }

        pub(crate) fn wake(&self) {
            let _ = UnixStream::connect(&self.path);
        }
    }

    impl Drop for Listener {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
        }
    }

    /// 本机控制通道（S4b）：`$TMPDIR/feiyin-<tag>-<pid>.sock`，0600。
    pub(crate) struct CtlListener {
        path: PathBuf,
        inner: UnixListener,
    }

    impl CtlListener {
        pub(crate) fn bind(tag: &str) -> io::Result<Self> {
            let path = std::env::temp_dir().join(format!(
                "feiyin-{}-{}.sock",
                tag.to_ascii_lowercase(),
                std::process::id()
            ));
            let _ = std::fs::remove_file(&path);
            let inner = UnixListener::bind(&path)?;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
            Ok(Self { path, inner })
        }

        pub(crate) fn endpoint(&self) -> &str {
            self.path.to_str().unwrap_or_default()
        }

        pub(crate) fn accept(&self) -> io::Result<UnixStream> {
            Ok(self.inner.accept()?.0)
        }
    }

    impl Drop for CtlListener {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}
