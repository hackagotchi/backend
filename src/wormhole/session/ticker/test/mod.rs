use hcor::{wormhole::RudeNote, Note, UPDATES_PER_SECOND};
use log::*;
use std::time::{Duration, Instant};
use tokio::time::{interval, timeout};

mod plant_yield;
mod rub_effect;

#[test]
pub fn wormhole_two_systems_connect() {
    use actix::System;
    use hcor::Hackstead;

    // attempt to establish logging, do nothing if it fails
    // (it probably fails because it's already been established in another test)
    drop(pretty_env_logger::try_init());

    let t1 = std::thread::spawn(move || {
        System::new("test").block_on(async move {
            let stead = Hackstead::register().await.unwrap();
            std::thread::sleep(Duration::from_millis(200));
            stead.slaughter().await.unwrap();
        });
    });

    let t2 = std::thread::spawn(move || {
        System::new("test").block_on(async move {
            let stead = Hackstead::register().await.unwrap();
            std::thread::sleep(Duration::from_millis(400));
            stead.slaughter().await.unwrap();
        });
    });

    t1.join().unwrap();
    t2.join().unwrap();
}

#[test]
pub fn wormhole_disconnect_reconnect() {
    use actix::System;
    use hcor::{wormhole, Hackstead};

    // attempt to establish logging, do nothing if it fails
    // (it probably fails because it's already been established in another test)
    drop(pretty_env_logger::try_init());

    System::new("test").block_on(async {
        let stead = Hackstead::register().await.unwrap();
        wormhole::disconnect().await.unwrap();
        std::thread::sleep(Duration::from_millis(400));
        wormhole::connect(&stead).await.unwrap();
        stead.slaughter().await.unwrap();
    })
}

async fn true_or_timeout(
    desc: &'static str,
    expected_ticks: f32,
    mut f: impl FnMut(&RudeNote) -> bool + Send + 'static,
) {
    const ERR_MARGIN_SECS: f32 = 1.0;

    let expected_seconds = expected_ticks / *UPDATES_PER_SECOND as f32;
    let expected_duration =
        Duration::from_millis(((expected_seconds + ERR_MARGIN_SECS) * 1000.0) as u64);

    info!(
        "preparing to wait no more than {:.2} seconds for {}",
        expected_seconds + ERR_MARGIN_SECS,
        desc
    );

    let (stop_progress_counter, mut rx) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        let mut i = interval(Duration::from_millis(500));
        let ticks_per_log = *UPDATES_PER_SECOND as f32 / 2.0;
        let mut ticks_left = expected_ticks;
        loop {
            i.tick().await;

            if let Ok(_) = rx.try_recv() {
                debug!("stopping progress counter for {}!", desc);
                break;
            }

            let prog = 100.0 - (ticks_left / expected_ticks) * 100.0;
            if prog < 100.0 {
                if prog != 0.0 {
                    info!("{} estimated progress [{:.1}%]", desc, prog);
                }
            } else {
                warn!(
                    "waiting {:.2}% longer than expected for {}?",
                    prog - 100.0,
                    desc
                )
            }
            ticks_left -= ticks_per_log;
        }
    });

    let until_finish = hcor::wormhole::until(move |note| {
        debug!("note from wormhole: {:#?}", note);
        match note {
            Note::Rude(r) => f(r),
            _ => false,
        }
    });

    let before = Instant::now();
    timeout(expected_duration, until_finish)
        .await
        .unwrap_or_else(|_| panic!("timeout waiting for {} to finish", desc))
        .expect("wormhole error while waiting for true or timeout");
    info!(
        "done in {}s (expected {}s)",
        Instant::now().duration_since(before).as_secs_f32(),
        expected_seconds
    );

    stop_progress_counter
        .send(())
        .expect("couldn't stop progress counter");
}
