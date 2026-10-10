use std::{
    error::Error,
    io,
    sync::{Arc, mpsc},
    thread::{self, JoinHandle},
};

use smithay_clipboard::Clipboard;
use winit::{
    raw_window_handle::{HasDisplayHandle, RawDisplayHandle},
    window::Window,
};

pub struct SystemClipboard {
    request_sender: mpsc::Sender<ClipboardRequest>,
    worker: Option<JoinHandle<()>>,
    _window: Arc<Window>,
}

type PasteDelivery = Box<dyn FnOnce(io::Result<String>) + Send>;

enum ClipboardRequest {
    Copy(String),
    Paste(PasteDelivery),
    Exit,
}

impl SystemClipboard {
    pub fn new(window: Arc<Window>) -> Result<Self, Box<dyn Error>> {
        let RawDisplayHandle::Wayland(display) = window.display_handle()?.as_raw() else {
            return Err("ttyrantula requires a Wayland display".into());
        };

        // SAFETY: the Arc stored beside Clipboard keeps winit's wl_display alive until
        // after Clipboard is dropped. smithay-clipboard has no safe raw-handle adapter.
        let clipboard = unsafe { Clipboard::new(display.display.as_ptr()) };
        let (request_sender, request_receiver) = mpsc::channel();
        let worker = thread::Builder::new()
            .name("ttyrantula-clipboard".into())
            .spawn(move || run_clipboard_worker(clipboard, &request_receiver))?;

        Ok(Self {
            request_sender,
            worker: Some(worker),
            _window: window,
        })
    }

    pub fn copy(&self, text: String) -> io::Result<()> {
        self.send(ClipboardRequest::Copy(text))
    }

    pub fn request_text(
        &self,
        deliver: impl FnOnce(io::Result<String>) + Send + 'static,
    ) -> io::Result<()> {
        self.send(ClipboardRequest::Paste(Box::new(deliver)))
    }

    fn send(&self, request: ClipboardRequest) -> io::Result<()> {
        self.request_sender
            .send(request)
            .map_err(|_| io::Error::other("clipboard worker is unavailable"))
    }
}

impl Drop for SystemClipboard {
    fn drop(&mut self) {
        let _ = self.request_sender.send(ClipboardRequest::Exit);
        if self
            .worker
            .take()
            .is_some_and(|worker| worker.join().is_err())
        {
            eprintln!("clipboard worker panicked during shutdown");
        }
    }
}

fn run_clipboard_worker(clipboard: Clipboard, requests: &mpsc::Receiver<ClipboardRequest>) {
    while let Ok(request) = requests.recv() {
        match request {
            ClipboardRequest::Copy(text) => clipboard.store(text),
            ClipboardRequest::Paste(deliver) => deliver(clipboard.load()),
            ClipboardRequest::Exit => break,
        }
    }
}
