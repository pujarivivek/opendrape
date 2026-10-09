//! Open and Save dialogs. The app uses the system's own dialogs; tests script the answers, so
//! no window ever opens during testing.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, Sender, channel};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DialogKind {
    Open,
    Save,
}

#[derive(Clone, Debug)]
pub enum FileDialogs {
    /// The system's dialogs (macOS and Windows), shown without freezing the window.
    Native,
    /// Tests: each dialog takes the next answer; `None`, or no answers left, means Cancel.
    Scripted(Rc<RefCell<VecDeque<Option<PathBuf>>>>),
}

impl FileDialogs {
    pub fn scripted(answers: Vec<Option<PathBuf>>) -> Self {
        Self::Scripted(Rc::new(RefCell::new(answers.into())))
    }

    /// Every dialog is cancelled.
    pub fn always_cancel() -> Self {
        Self::Scripted(Rc::default())
    }

    /// Opens a dialog. The chosen path (or `None` for Cancel) arrives on the returned channel,
    /// with a repaint request.
    pub fn ask(
        &self,
        kind: DialogKind,
        suggested_name: &str,
        frame: &eframe::Frame,
        ctx: &egui::Context,
    ) -> Receiver<Option<PathBuf>> {
        let (tx, rx) = channel();
        match self {
            Self::Scripted(answers) => {
                let _ = tx.send(answers.borrow_mut().pop_front().flatten());
            }
            Self::Native => native(kind, suggested_name, frame, ctx.clone(), tx),
        }
        rx
    }
}

#[cfg(any(windows, target_os = "macos"))]
fn native(
    kind: DialogKind,
    suggested_name: &str,
    frame: &eframe::Frame,
    ctx: egui::Context,
    tx: Sender<Option<PathBuf>>,
) {
    use std::future::Future;
    use std::pin::Pin;
    let dialog = rfd::AsyncFileDialog::new()
        .add_filter(crate::tr!("file-type-project"), &[opendrape_io::EXTENSION])
        .set_parent(frame);
    // The dialog is created here, on the main thread; only the waiting happens elsewhere.
    let answer: Pin<Box<dyn Future<Output = Option<rfd::FileHandle>> + Send>> = match kind {
        DialogKind::Open => Box::pin(dialog.pick_file()),
        DialogKind::Save => Box::pin(dialog.set_file_name(suggested_name).save_file()),
    };
    std::thread::spawn(move || {
        let path = pollster::block_on(answer).map(|f| f.path().to_path_buf());
        let _ = tx.send(path);
        ctx.request_repaint();
    });
}

/// Linux builds have no file dialogs yet: every dialog is cancelled.
#[cfg(not(any(windows, target_os = "macos")))]
fn native(
    _: DialogKind,
    _: &str,
    _: &eframe::Frame,
    _: egui::Context,
    tx: Sender<Option<PathBuf>>,
) {
    let _ = tx.send(None);
}
