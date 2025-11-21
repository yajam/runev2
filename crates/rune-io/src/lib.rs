//! IO abstractions for loading resources and dialogs.

#![allow(clippy::all)]

use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::mpsc::{self, Receiver, TryRecvError},
    thread::{self, JoinHandle},
};

/// The type of file dialog to present to the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileDialogKind {
    OpenFile,
    OpenFiles,
}

/// Result emitted once a file dialog completes.
#[derive(Debug)]
pub struct FileDialogResult {
    pub request_id: u64,
    pub path: Option<PathBuf>,
    pub paths: Option<Vec<PathBuf>>,
}

struct PendingDialog {
    request_id: u64,
    receiver: Receiver<Option<Vec<PathBuf>>>,
    join: Option<JoinHandle<()>>,
}

/// Manages asynchronous file dialog requests without blocking the event loop.
pub struct FileDialogService {
    pending: Vec<PendingDialog>,
}

impl FileDialogService {
    /// Create a new service instance.
    pub fn new() -> Self {
        Self {
            pending: Vec::new(),
        }
    }

    /// Request that a dialog of `kind` be shown. The result will become
    /// available on a future call to [`poll`].
    pub fn request(&mut self, request_id: u64, kind: FileDialogKind) {
        let (tx, rx) = mpsc::channel();

        let join = thread::spawn(move || {
            let selection: Option<Vec<PathBuf>> = match kind {
                FileDialogKind::OpenFile => rfd::FileDialog::new().pick_file().map(|p| vec![p]),
                FileDialogKind::OpenFiles => rfd::FileDialog::new().pick_files(),
            };

            let _ = tx.send(selection);
        });

        self.pending.push(PendingDialog {
            request_id,
            receiver: rx,
            join: Some(join),
        });
    }

    /// Poll for dialog completions, returning all results that are ready.
    pub fn poll(&mut self) -> Vec<FileDialogResult> {
        let mut ready = Vec::new();
        let mut still_pending = Vec::new();

        for mut dialog in self.pending.drain(..) {
            match dialog.receiver.try_recv() {
                Ok(paths_opt) => {
                    if let Some(join) = dialog.join.take() {
                        let _ = join.join();
                    }
                    let (path, paths) = match paths_opt {
                        Some(list) => (list.get(0).cloned(), Some(list)),
                        None => (None, None),
                    };
                    ready.push(FileDialogResult {
                        request_id: dialog.request_id,
                        path,
                        paths,
                    });
                }
                Err(TryRecvError::Empty) => {
                    still_pending.push(dialog);
                }
                Err(TryRecvError::Disconnected) => {
                    if let Some(join) = dialog.join.take() {
                        let _ = join.join();
                    }
                    ready.push(FileDialogResult {
                        request_id: dialog.request_id,
                        path: None,
                        paths: None,
                    });
                }
            }
        }

        self.pending = still_pending;
        ready
    }

    /// True if there are outstanding dialogs waiting to complete.
    pub fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }
}

// -------------------------- HTTP SERVICE --------------------------

#[derive(Debug)]
pub struct HttpResult {
    pub request_id: u64,
    pub status: Option<i32>,
    pub content_type: Option<String>,
    pub body: Option<String>,
    pub error: Option<String>,
}

struct PendingHttp {
    request_id: u64,
    receiver: Receiver<HttpResult>,
    join: Option<JoinHandle<()>>,
}

/// Manages non-blocking HTTP requests using a worker thread per request.
pub struct HttpService {
    pending: Vec<PendingHttp>,
    allowed_origins: Option<HashSet<String>>, // e.g., "https://example.com:443"
    default_timeout: std::time::Duration,
}

impl HttpService {
    pub fn new() -> Self {
        Self {
            pending: Vec::new(),
            allowed_origins: None,
            default_timeout: std::time::Duration::from_secs(15),
        }
    }

    /// Configure an allowlist of origins for outbound HTTP(S). When set, any
    /// request to an origin not listed will be blocked with an error result.
    pub fn set_origin_allowlist<I: IntoIterator<Item = String>>(&mut self, origins: I) {
        self.allowed_origins = Some(origins.into_iter().collect());
    }

    /// Remove any origin allowlist, allowing all outbound requests.
    pub fn clear_origin_allowlist(&mut self) {
        self.allowed_origins = None;
    }

    /// Set the default timeout used for requests when a per-request timeout is not provided.
    pub fn set_default_timeout(&mut self, timeout: std::time::Duration) {
        self.default_timeout = timeout;
    }

    /// Cancel a request by id: drop the pending entry so any eventual result is ignored.
    pub fn cancel(&mut self, request_id: u64) {
        self.pending.retain(|p| p.request_id != request_id);
    }

    pub fn request(
        &mut self,
        request_id: u64,
        method: &str,
        url: &str,
        headers: Option<HashMap<String, String>>,
        body: Option<String>,
        timeout: Option<std::time::Duration>,
    ) {
        let (tx, rx) = mpsc::channel();
        let method = method.to_string();
        let url_str = url.to_string();
        let timeout = timeout.unwrap_or(self.default_timeout);

        // Evaluate allowlist and scheme guard before spawning worker
        let allowlist = self.allowed_origins.clone();
        let join = thread::spawn(move || {
            let parsed = reqwest::Url::parse(&url_str);
            // Scheme + origin guards
            if let Ok(ref u) = parsed {
                let scheme = u.scheme().to_ascii_lowercase();
                if scheme != "http" && scheme != "https" {
                    let _ = tx.send(HttpResult {
                        request_id,
                        status: None,
                        content_type: None,
                        body: None,
                        error: Some("blocked: unsupported scheme".into()),
                    });
                    return;
                }
                if let Some(set) = allowlist.as_ref() {
                    let origin = match u.port_or_known_default() {
                        Some(port) => {
                            format!("{}://{}:{}", u.scheme(), u.host_str().unwrap_or(""), port)
                        }
                        None => format!("{}://{}", u.scheme(), u.host_str().unwrap_or("")),
                    };
                    if !set.contains(&origin) {
                        let _ = tx.send(HttpResult {
                            request_id,
                            status: None,
                            content_type: None,
                            body: None,
                            error: Some("blocked: origin not allowed".into()),
                        });
                        return;
                    }
                }
            }

            let result = match parsed {
                Ok(u) => {
                    match reqwest::blocking::Client::builder()
                        .user_agent("RuneBrowser/0.1")
                        .timeout(timeout)
                        .build()
                    {
                        Ok(client) => {
                            let mut req = match method.to_ascii_lowercase().as_str() {
                                "get" => client.get(u),
                                "post" => client.post(u),
                                "put" => client.put(u),
                                "patch" => client.patch(u),
                                "delete" => client.delete(u),
                                _ => client.get(u),
                            };
                            if let Some(hs) = headers.as_ref() {
                                for (k, v) in hs.iter() {
                                    if let Ok(name) =
                                        reqwest::header::HeaderName::from_bytes(k.as_bytes())
                                    {
                                        req = req.header(name, v.clone());
                                    }
                                }
                            }
                            if let Some(b) = body.as_ref() {
                                req = req.body(b.clone());
                            }
                            match req.send() {
                                Ok(resp) => {
                                    let status = Some(resp.status().as_u16() as i32);
                                    let ct = resp
                                        .headers()
                                        .get(reqwest::header::CONTENT_TYPE)
                                        .and_then(|v| v.to_str().ok())
                                        .map(|s| s.to_string());
                                    let text = resp.text().ok();
                                    HttpResult {
                                        request_id,
                                        status,
                                        content_type: ct,
                                        body: text,
                                        error: None,
                                    }
                                }
                                Err(err) => HttpResult {
                                    request_id,
                                    status: None,
                                    content_type: None,
                                    body: None,
                                    error: Some(err.to_string()),
                                },
                            }
                        }
                        Err(err) => HttpResult {
                            request_id,
                            status: None,
                            content_type: None,
                            body: None,
                            error: Some(err.to_string()),
                        },
                    }
                }
                Err(err) => HttpResult {
                    request_id,
                    status: None,
                    content_type: None,
                    body: None,
                    error: Some(err.to_string()),
                },
            };
            let _ = tx.send(result);
        });

        self.pending.push(PendingHttp {
            request_id,
            receiver: rx,
            join: Some(join),
        });
    }

    pub fn poll(&mut self) -> Vec<HttpResult> {
        let mut ready = Vec::new();
        let mut still = Vec::new();
        for mut pending in self.pending.drain(..) {
            match pending.receiver.try_recv() {
                Ok(res) => {
                    if let Some(j) = pending.join.take() {
                        let _ = j.join();
                    }
                    ready.push(res);
                }
                Err(TryRecvError::Empty) => still.push(pending),
                Err(TryRecvError::Disconnected) => {
                    if let Some(j) = pending.join.take() {
                        let _ = j.join();
                    }
                    ready.push(HttpResult {
                        request_id: pending.request_id,
                        status: None,
                        content_type: None,
                        body: None,
                        error: Some("disconnected".into()),
                    });
                }
            }
        }
        self.pending = still;
        ready
    }

    pub fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }
}
