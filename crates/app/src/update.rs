//! Self-update with Velopack from LittleDraw's GitHub releases. Only an
//! installed copy (the AppImage or the Windows setup) updates itself; a build
//! run from the repository skips all of this.
//!
//! A newer release is downloaded in the background and installed when the
//! app closes. If that fails, Velopack installs it on the next start.

use std::sync::mpsc::{self, Receiver};

use velopack::sources::GithubSource;
use velopack::{UpdateCheck, UpdateInfo, UpdateManager, VelopackApp};
use winit::event_loop::EventLoopProxy;

const REPO_URL: &str = "https://github.com/im4dev/LittleDraw";

/// Handles the installer's and updater's calls into the app (they start it
/// with special arguments and expect it to exit), so it must run first.
pub fn run_hooks() {
    VelopackApp::build().run();
}

struct Ready {
    manager: UpdateManager,
    info: Box<UpdateInfo>,
}

pub struct Updates {
    rx: Receiver<Ready>,
    ready: Option<Ready>,
}

impl Updates {
    /// Starts looking for an update in the background; `proxy` wakes the
    /// event loop once one is downloaded. `None` when not installed.
    pub fn start(proxy: EventLoopProxy) -> Option<Self> {
        let source = GithubSource::new(REPO_URL, None, false);
        let manager = UpdateManager::new(source, None, None).ok()?;
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || match download(&manager) {
            Ok(Some(info)) => {
                if tx.send(Ready { manager, info }).is_ok() {
                    proxy.wake_up();
                }
            }
            Ok(None) => {}
            Err(err) => eprintln!("could not update LittleDraw: {err}"),
        });
        Some(Updates { rx, ready: None })
    }

    /// The version that has just finished downloading, if any.
    pub fn poll(&mut self) -> Option<String> {
        let ready = self.rx.try_recv().ok()?;
        let version = ready.info.TargetFullRelease.Version.clone();
        self.ready = Some(ready);
        Some(version)
    }

    /// Has Velopack install the downloaded update once this process exits.
    pub fn install_on_exit(&self) {
        let Some(Ready { manager, info }) = &self.ready else {
            return;
        };
        if let Err(err) = manager.wait_exit_then_apply_updates(info.as_ref(), true, false, Vec::<String>::new()) {
            eprintln!("could not install the update: {err}");
        }
    }
}

fn download(manager: &UpdateManager) -> Result<Option<Box<UpdateInfo>>, velopack::Error> {
    let UpdateCheck::UpdateAvailable(info) = manager.check_for_updates()? else {
        return Ok(None);
    };
    manager.download_updates(&info, None)?;
    Ok(Some(info))
}
