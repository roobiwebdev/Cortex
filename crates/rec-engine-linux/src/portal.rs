//! SOURCE OF TRUTH KEYWORDS: ScreenCast portal, ashpd, PipeWire fd, restore token, PortalStream, pick, cursor embedded
//! WHAT:  Asks xdg-desktop-portal for a screen or window (`SelectSources` + `Start`), with the
//!        cursor drawn into the frames and a restore token so the system picker is skipped
//!        next time, and opens the PipeWire remote. `PortalStream` keeps the portal session
//!        open until it is dropped.
//! WHY:   On Wayland only the portal may capture the screen, and only what the user picked in
//!        the system dialog. GStreamer's `pipewiresrc` reads the stream from the fd and node
//!        id. The portal session lives on its own thread (zbus is async), which also closes
//!        it when the recording ends.
//! WHERE: `engine` (before a recording or its countdown).

use std::os::fd::{AsRawFd, OwnedFd};
use std::sync::mpsc;
use std::time::Duration;

use ashpd::desktop::PersistMode;
use ashpd::desktop::screencast::{CursorMode, Screencast, SelectSourcesOptions, SourceType};

use crate::portal_state::PickKind;

/// A running portal capture. Dropping it closes the portal session.
pub struct PortalStream {
    fd: OwnedFd,
    pub node: u32,
    /// The stream's size as the portal reports it (logical pixels; may be absent).
    pub size: Option<(u32, u32)>,
    close: Option<mpsc::Sender<()>>,
}

impl PortalStream {
    /// `pipewiresrc` text for this stream. `pipewiresrc` duplicates the fd, so this one is
    /// only closed with the stream.
    pub fn source(&self) -> String {
        format!(
            "pipewiresrc fd={} path={} do-timestamp=true keepalive-time=1000",
            self.fd.as_raw_fd(),
            self.node
        )
    }
}

impl Drop for PortalStream {
    fn drop(&mut self) {
        if let Some(tx) = self.close.take() {
            let _ = tx.send(());
        }
    }
}

/// What a pick returns: the stream and the token to store for next time.
pub struct Picked {
    pub stream: PortalStream,
    pub restore_token: Option<String>,
}

fn describe(e: &ashpd::Error) -> String {
    match e {
        ashpd::Error::Response(ashpd::desktop::ResponseError::Cancelled) => {
            "Screen sharing was cancelled".into()
        }
        ashpd::Error::Portal(p) => format!("The screen-sharing portal refused: {p}"),
        other => format!(
            "Could not reach the screen-sharing portal (is xdg-desktop-portal running?): {other}"
        ),
    }
}

/// Ask for a monitor or a window. Blocks until the user answers the system dialog (or the
/// restore token is accepted silently). `timeout` bounds the wait.
pub fn pick(
    kind: PickKind,
    cursor: bool,
    token: Option<String>,
    timeout: Duration,
) -> Result<Picked, String> {
    let (tx, rx) =
        mpsc::channel::<Result<(OwnedFd, u32, Option<(u32, u32)>, Option<String>), String>>();
    let (close_tx, close_rx) = mpsc::channel::<()>();
    std::thread::Builder::new()
        .name("screencast-portal".into())
        .spawn(move || {
            pollster::block_on(async move {
                let run = async {
                    let proxy = Screencast::new().await?;
                    let session = proxy.create_session(Default::default()).await?;
                    let sources = match kind {
                        PickKind::Monitor => SourceType::Monitor,
                        PickKind::Window => SourceType::Window,
                    };
                    // Embedded draws the pointer into the frames; fall back when unsupported.
                    let modes = proxy.available_cursor_modes().await.unwrap_or_default();
                    let cursor_mode = if cursor && modes.contains(CursorMode::Embedded) {
                        CursorMode::Embedded
                    } else {
                        CursorMode::Hidden
                    };
                    let options = SelectSourcesOptions::default()
                        .set_cursor_mode(cursor_mode)
                        .set_sources(ashpd::enumflags2::BitFlags::from(sources))
                        .set_multiple(false)
                        .set_persist_mode(PersistMode::ExplicitlyRevoked)
                        .set_restore_token(token.as_deref());
                    proxy.select_sources(&session, options).await?.response()?;
                    let streams = proxy
                        .start(&session, None, Default::default())
                        .await?
                        .response()?;
                    let restore = streams.restore_token().map(str::to_string);
                    let Some(stream) = streams.streams().first() else {
                        return Err(ashpd::Error::NoResponse);
                    };
                    let node = stream.pipe_wire_node_id();
                    let size = stream
                        .size()
                        .and_then(|(w, h)| Some((u32::try_from(w).ok()?, u32::try_from(h).ok()?)));
                    let fd = proxy
                        .open_pipe_wire_remote(&session, Default::default())
                        .await?;
                    Ok::<_, ashpd::Error>((fd, node, size, restore, session))
                };
                match run.await {
                    Ok((fd, node, size, restore, session)) => {
                        let _ = tx.send(Ok((fd, node, size, restore)));
                        // Keep the portal session until the recording is over.
                        let _ = close_rx.recv();
                        let _ = session.close().await;
                    }
                    Err(e) => {
                        let _ = tx.send(Err(describe(&e)));
                    }
                }
            });
        })
        .map_err(|e| format!("Could not start the portal thread: {e}"))?;
    match rx.recv_timeout(timeout) {
        Ok(Ok((fd, node, size, restore_token))) => Ok(Picked {
            stream: PortalStream {
                fd,
                node,
                size,
                close: Some(close_tx),
            },
            restore_token,
        }),
        Ok(Err(e)) => Err(e),
        Err(_) => Err("The screen-sharing dialog did not answer in time".into()),
    }
}
