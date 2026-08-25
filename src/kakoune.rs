use std::io::{self, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;

pub fn kak_quote(input: &str) -> String {
    input.replace('\'', "''")
}

/// Candidate paths for a Kakoune session's unix socket, in priority order.
pub fn session_socket_paths(session: &str) -> Vec<PathBuf> {
    let session = session.trim();
    let mut candidate_paths: Vec<PathBuf> = Vec::new();

    if let Ok(session_dir) = std::env::var("KAKOUNE_SESSION_DIR") {
        candidate_paths.push(PathBuf::from(session_dir).join(session));
    }
    if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        candidate_paths.push(PathBuf::from(runtime_dir).join("kakoune").join(session));
    }
    if let Ok(user) = std::env::var("USER") {
        candidate_paths.push(PathBuf::from(format!("/tmp/kakoune-{user}/{session}")));
        candidate_paths.push(PathBuf::from(format!("/var/tmp/kakoune-{user}/{session}")));
    }
    candidate_paths.push(PathBuf::from(format!("/tmp/kakoune/{session}")));

    candidate_paths
}

/// Send a raw command string to a Kakoune session by writing it directly to
/// the session socket, using the same framed wire protocol as `kak -p`.
///
/// Kakoune closes the connection after each command message, so this cannot
/// be turned into a persistent connection, but it still avoids the fork/exec
/// of the kak binary per update.
pub fn send_command_to_session(session: &str, command: &str) -> io::Result<()> {
    const MESSAGE_TYPE_COMMAND: u8 = 2;

    let mut frame = Vec::with_capacity(command.len() + 9);
    frame.push(MESSAGE_TYPE_COMMAND);
    frame.extend_from_slice(&((command.len() + 9) as u32).to_ne_bytes());
    frame.extend_from_slice(&(command.len() as u32).to_ne_bytes());
    frame.extend_from_slice(command.as_bytes());

    for path in session_socket_paths(session) {
        if !path.exists() {
            continue;
        }
        match UnixStream::connect(&path) {
            Ok(mut stream) => return stream.write_all(&frame),
            Err(err) => {
                log::debug!("send_command_to_session: connect to {} failed: {err}", path.display());
            }
        }
    }

    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!("no reachable socket for session '{session}'"),
    ))
}

