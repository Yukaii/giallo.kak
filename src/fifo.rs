use std::fs;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::thread;

use giallo::Registry;
use log;

use crate::config::Config;
use crate::highlight::{highlight_and_send, BufferContext};

pub fn create_fifo(path: &Path) -> io::Result<()> {
    let c_path = std::ffi::CString::new(path.as_os_str().to_string_lossy().as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid fifo path"))?;
    let ret = unsafe { libc::mkfifo(c_path.as_ptr(), 0o644) };
    if ret != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

pub fn open_fifo_nonblocking(path: &Path) -> io::Result<std::fs::File> {
    let c_path = std::ffi::CString::new(path.as_os_str().to_string_lossy().as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid fifo path"))?;

    let fd = unsafe { libc::open(c_path.as_ptr(), libc::O_RDONLY | libc::O_NONBLOCK, 0o644) };

    if fd < 0 {
        return Err(io::Error::last_os_error());
    }

    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL, 0) };
    if flags < 0 {
        unsafe { libc::close(fd) };
        return Err(io::Error::last_os_error());
    }

    let ret = unsafe { libc::fcntl(fd, libc::F_SETFL, flags & !libc::O_NONBLOCK) };
    if ret < 0 {
        unsafe { libc::close(fd) };
        return Err(io::Error::last_os_error());
    }

    let file = unsafe { std::fs::File::from_raw_fd(fd) };
    Ok(file)
}

pub fn handle_init(token: &str, base_dir: &Path) -> io::Result<(std::path::PathBuf, String)> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    fs::create_dir_all(base_dir)?;
    let mut hasher = DefaultHasher::new();
    token.hash(&mut hasher);
    let hash = format!("{:x}", hasher.finish());
    let req = base_dir.join(format!("{hash}.req.fifo"));
    let sentinel = format!("giallo-{hash}");

    if !req.exists() {
        create_fifo(&req)?;
    }

    Ok((req, sentinel))
}

pub fn run_buffer_fifo(
    req_path: &Path,
    registry: &Registry,
    config: &Config,
    ctx: BufferContext,
    quit_flag: Option<&Arc<AtomicBool>>,
) -> io::Result<()> {
    log::debug!(
        "buffer FIFO: starting for buffer={} sentinel={}",
        ctx.buffer,
        ctx.sentinel
    );

    let (tx, rx): (Sender<String>, Receiver<String>) = channel();

    let ctx_clone = ctx.clone();
    let quit_flag_clone = quit_flag.map(|f| f.clone());
    let req_path_owned = req_path.to_path_buf();

    let reader_handle = thread::spawn(move || {
        let mut buf = String::new();
        let sentinel = ctx_clone.sentinel.clone();

        let mut file = match open_fifo_nonblocking(&req_path_owned) {
            Ok(f) => f,
            Err(err) => {
                log::error!("reader: failed to open FIFO: {}", err);
                return;
            }
        };

        loop {
            if let Some(ref flag) = quit_flag_clone {
                if flag.load(Ordering::Relaxed) {
                    break;
                }
            }

            let fd = file.as_raw_fd();
            let mut poll_fd = libc::pollfd {
                fd,
                events: libc::POLLIN,
                revents: 0,
            };
            let poll_result = unsafe { libc::poll(&mut poll_fd, 1, 250) };
            if poll_result < 0 {
                log::warn!("reader: poll error: {}", io::Error::last_os_error());
                thread::sleep(std::time::Duration::from_millis(50));
                continue;
            }
            if poll_result == 0 {
                continue;
            }
            if poll_fd.revents & libc::POLLIN == 0 {
                if poll_fd.revents & libc::POLLHUP != 0 {
                    thread::sleep(std::time::Duration::from_millis(100));
                }
                continue;
            }

            let mut read_buf = String::new();
            match std::io::Read::read_to_string(&mut file, &mut read_buf) {
                Ok(0) => {
                    thread::sleep(std::time::Duration::from_millis(50));
                    continue;
                }
                Ok(_) => {
                    buf.push_str(&read_buf);
                }
                Err(err) => {
                    if err.kind() == io::ErrorKind::WouldBlock {
                        thread::sleep(std::time::Duration::from_millis(5));
                        continue;
                    } else {
                        log::warn!("reader: read error: {}", err);
                        thread::sleep(std::time::Duration::from_millis(50));
                        continue;
                    }
                }
            }

            while let Some(index) = buf.find(&sentinel) {
                let content = buf[..index].to_string();
                let end_index = index + sentinel.len();
                buf.drain(..end_index);

                if tx.send(content).is_err() {
                    log::debug!("reader: channel closed, exiting");
                    return;
                }
            }
        }
    });

    loop {
        if let Some(flag) = quit_flag {
            if flag.load(Ordering::Relaxed) {
                drop(rx);
                let _ = reader_handle.join();
                break;
            }
        }

        match rx.recv_timeout(std::time::Duration::from_millis(100)) {
            Ok(content) => {
                let lang = ctx.lang.lock().unwrap().clone();
                let theme = ctx.theme.lock().unwrap().clone();

                log::debug!(
                    "processor: received buffer (lang={} theme={} len={})",
                    lang,
                    theme,
                    content.len()
                );

                if !lang.is_empty() {
                    highlight_and_send(&content, &lang, &theme, registry, config, &ctx);
                } else {
                    log::warn!(
                        "processor: empty language, skipping highlight for buffer={}",
                        ctx.buffer
                    );
                }
            }
            Err(_) => {
                continue;
            }
        }
    }

    log::debug!("buffer FIFO: exiting for buffer={}", ctx.buffer);
    Ok(())
}
