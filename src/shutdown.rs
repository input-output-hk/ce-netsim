use tokio::sync::watch;

/// Controller to send a signal to the [`ShutdownReceiver`]s
/// that it is time to attempt a clean shutdown.
///
/// Once a shutdown has been sent it is no longer possible
/// to cancel the shutdown and recall the signal.
///
pub struct ShutdownController(watch::Sender<bool>);

/// Receiving end of the _clean shutdown_ signal.
///
#[derive(Clone)]
pub struct ShutdownReceiver(watch::Receiver<bool>);

impl ShutdownController {
    pub fn new() -> Self {
        let (a, _) = watch::channel(false);
        Self(a)
    }

    /// subscribe to the shutdown signal
    pub fn subscribe(&self) -> ShutdownReceiver {
        ShutdownReceiver(self.0.subscribe())
    }

    /// consume the controller and send the signal to the
    /// [`ShutdownReceiver`]s that it is time to attempt
    /// a clean shutdown.
    pub fn shutdown(self) {
        self.0.send_replace(true);
    }
}

impl ShutdownReceiver {
    /// Listen for a clean shutdown
    ///
    /// Returns `true` is a clean shutdown signal has been initiated.
    /// A clean shutdown may also be sent if the [`ShutdownController`]
    /// has been dropped (i.e. the channel is closed).
    pub async fn is_shutting_down(&mut self) -> bool {
        self.0.wait_for(|b| *b).await.map(|r| *r).unwrap_or(true)
    }
}
