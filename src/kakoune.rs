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

