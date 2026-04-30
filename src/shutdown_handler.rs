use futures_util::StreamExt;
use log::info;
use tokio::sync::mpsc;
use zbus::Connection;
use logind_zbus::manager::ManagerProxy;

pub enum ShutdownSignal {
    PrepareForSleep { start: bool },
    PrepareForShutdown { start: bool },
}

pub struct ShutdownHandler {
    tx: mpsc::Sender<ShutdownSignal>,
}

impl ShutdownHandler {
    pub fn new(tx: mpsc::Sender<ShutdownSignal>) -> Self {
        Self { tx }
    }

    pub async fn run(&self) -> zbus::Result<()> {
        let connection = Connection::system().await?;
        let manager = ManagerProxy::new(&connection).await?;

        let sleep_fd = Some(manager.inhibit(
            logind_zbus::manager::InhibitType::Sleep,
            "mono-tracker",
            "Close sessions before sleep",
            "delay",
        ).await?);
        let shutdown_fd = Some(manager.inhibit(
            logind_zbus::manager::InhibitType::Shutdown,
            "mono-tracker",
            "Close sessions before shutdown",
            "delay",
        ).await?);

        info!("Delay inhibitor locks taken for sleep and shutdown");

        let mut sleep_stream = manager.receive_prepare_for_sleep().await?;
        let mut shutdown_stream = manager.receive_prepare_for_shutdown().await?;

        let mut sleep_fd = sleep_fd;
        let mut shutdown_fd = shutdown_fd;

        loop {
            tokio::select! {
                Some(signal) = sleep_stream.next() => {
                    if let Ok(args) = signal.args() {
                        self.tx.send(ShutdownSignal::PrepareForSleep { start: args.start }).await.ok();
                        if args.start {
                            sleep_fd.take();
                        }
                    }
                }
                Some(signal) = shutdown_stream.next() => {
                    if let Ok(args) = signal.args() {
                        self.tx.send(ShutdownSignal::PrepareForShutdown { start: args.start }).await.ok();
                        if args.start {
                            shutdown_fd.take();
                        }
                    }
                }
                else => break,
            }
        }

        Ok(())
    }
}
