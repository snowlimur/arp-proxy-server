use sd_notify::NotifyState;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::{env, process};
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::Notify;
use tokio::time::sleep;
use tracing::{error, info};

pub fn run(notifier: Arc<Notify>) {
    tokio::spawn(handle_signals(notifier.clone()));
    tokio::spawn(async move {
        // @todo make it configurable
        sleep(Duration::from_secs(1)).await;
        let r = sd_notify::notify(false, &[NotifyState::Ready]);
        if let Err(e) = r {
            error!("notify ready: {}", e);
        }
    });
}

async fn handle_signals(notifier: Arc<Notify>) {
    let mut interrupt = signal(SignalKind::interrupt()).unwrap();
    let mut terminate = signal(SignalKind::terminate()).unwrap();
    let mut quit = signal(SignalKind::quit()).unwrap();
    let mut hup = signal(SignalKind::hangup()).unwrap();

    let mut sigint = false;
    let mut sighup = false;
    tokio::select! {
        _ = interrupt.recv() => {
            info!("received interrupt signal");
            sigint = true;
        },
        _ = hup.recv() => {
            info!("received hup signal");
            sighup = true;
        },
        _ = terminate.recv() => {
            info!("received terminate signal");
        },
        _ = quit.recv() => {
            info!("received quit signal");
        },
    }

    if sighup {
        let pid = fork();
        if let Err(e) = pid {
            info!("fork: {}", e);
            process::exit(1);
        }

        let pid = pid.unwrap();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before Unix epoch");
        let r = sd_notify::notify(
            false,
            &[
                NotifyState::Reloading,
                NotifyState::MonotonicUsec(now.as_micros() as i128),
                NotifyState::MainPid(pid),
            ],
        );
        if let Err(e) = r {
            error!("notify reloading: {}", e);
        }

        exit_after(Duration::from_secs(30)).await;
    } else {
        let _ = sd_notify::notify(true, &[NotifyState::Stopping]);
    }

    notifier.notify_waiters();
    if sigint {
        process::exit(0);
    }
}

async fn exit_after(duration: Duration) {
    sleep(duration).await;
    process::exit(0);
}

fn fork() -> Result<u32, Box<dyn std::error::Error>> {
    let exe = env::current_exe()?;
    let child = Command::new(exe)
        .args(env::args().skip(1))
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?;

    Ok(child.id())
}
