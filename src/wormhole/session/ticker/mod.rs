use super::{SessSend, SessSendSubmit};
use hcor::{plant, wormhole::RudeNote, Hackstead, IdentifiesTile, Note};

mod finish;
use finish::finish_timer;

#[cfg(all(test, feature = "hcor_client"))]
mod test;

#[derive(Default, Clone, actix::MessageResponse)]
pub struct Ticker {
    pub timers: Vec<plant::ServerTimer>,
    complete_timers: Vec<(usize, plant::timer::Lifecycle)>,
}

impl Ticker {
    pub fn new(hs: &mut Hackstead) -> Self {
        Self {
            timers: hs
                .timers()
                .map(|st| plant::ServerTimer {
                    timer_id: st.timer_id,
                    tile_id: st.tile_id,
                    kind: st.kind,
                    value: st.duration,
                    predicted_next: 0.0,
                })
                .collect(),
            complete_timers: vec![],
        }
    }

    pub fn start(
        &mut self,
        ss: &mut SessSend,
        timer: plant::SharedTimer,
    ) -> Result<(), hcor::id::NoSuch> {
        ss.plant_mut(timer.tile_id)?.timers.push(timer);
        self.timers.push(plant::ServerTimer {
            timer_id: timer.timer_id,
            tile_id: timer.tile_id,
            kind: timer.kind,
            value: timer.duration,
            predicted_next: 0.0,
        });
        Ok(())
    }

    pub fn xp(&self, td: impl IdentifiesTile) -> f32 {
        let tile_id = td.tile_id();
        self.timers
            .iter()
            .find(|t| t.tile_id == tile_id && t.kind == plant::TimerKind::Xp)
            .map(|t| t.value)
            .unwrap_or(0.0)
    }

    pub fn increase_xp(&mut self, td: impl IdentifiesTile, amt: f32) -> Result<(), ()> {
        let tile_id = td.tile_id();
        if let Some(t) = self
            .timers
            .iter_mut()
            .find(|t| t.tile_id == tile_id && t.kind == plant::TimerKind::Xp)
        {
            t.value += amt;
            Ok(())
        } else {
            Err(())
        }
    }

    pub fn tick(&mut self, ss: &mut SessSend) -> SessSendSubmit {
        let buff_book = ss.buff_book();

        for (i, timer) in &mut self.timers.iter_mut().enumerate() {
            let shartimer = match ss.timer(timer.timer_id) {
                Some(t) => t.clone(),
                None => {
                    log::error!("Server timer missing Shared counterpart: {:#?}", timer);
                    continue;
                }
            };

            let buffs = buff_book.page(shartimer.tile_id);
            let delta = match &timer.kind {
                plant::TimerKind::Yield => {
                    buffs.yield_speed_multiplier * buffs.total_extra_time_ticks as f32
                }
                plant::TimerKind::Craft { .. } => {
                    buffs.craft_speed_multiplier * buffs.total_extra_time_ticks as f32
                }
                plant::TimerKind::Rub { .. } => 1.0,
                plant::TimerKind::Xp => 1.0,
            };
            timer.value -= delta;
            if timer.predicted_next != timer.value {
                ss.send_note(Note::Rude(RudeNote::TimerUpdate {
                    timer_id: timer.timer_id,
                    value: timer.value,
                    rate: delta,
                }));
            }
            timer.predicted_next = timer.value - delta;

            if timer.value <= 0.0 {
                self.complete_timers.push((i, shartimer.lifecycle));
            }
        }

        for (i, lifecycle) in self.complete_timers.drain(..) {
            use plant::timer::Lifecycle;

            let servtim = match lifecycle {
                Lifecycle::Perennial { duration } => {
                    let t = self.timers.get_mut(i).unwrap();
                    t.value = duration;
                    *t
                }
                Lifecycle::Annual => self.timers.swap_remove(i),
            };
            let shartim = match ss.take_timer(servtim.timer_id) {
                Some(t) => t,
                None => {
                    log::error!(
                        "server timer references missing shared timer: {:#?}",
                        servtim
                    );
                    continue;
                }
            };

            match finish_timer(ss, shartim, servtim) {
                Ok(Some(n)) => ss.send_note(Note::Rude(n)),
                Err(e) => log::error!("error finishing timer {:#?}: {}", shartim, e),
                _ => {}
            }
        }

        SessSendSubmit::Submit
    }
}
