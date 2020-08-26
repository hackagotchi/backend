use super::{SessSend, SessSendSubmit};
use hcor::{plant, Hackstead, Note};

mod finish;
use finish::finish_timer;

#[cfg(all(test, feature = "hcor_client"))]
mod test;

#[derive(Default)]
pub struct Ticker {
    pub timers: Vec<plant::Timer>,
    complete_timers: Vec<(usize, plant::timer::Lifecycle)>,
}

impl Ticker {
    pub fn new(hs: &mut Hackstead) -> Self {
        Self {
            timers: hs.timers.drain(..).collect(),
            complete_timers: vec![],
        }
    }

    pub fn start(&mut self, timer: plant::Timer) {
        self.timers.push(timer);
    }

    pub fn tick(&mut self, ss: &mut SessSend) -> SessSendSubmit {
        for (i, t) in &mut self.timers.iter_mut().enumerate() {
            t.until_finish -= 1.0;

            if t.until_finish <= 0.0 {
                self.complete_timers.push((i, t.lifecycle));
            }
        }

        for (i, lifecycle) in self.complete_timers.drain(..) {
            use plant::timer::Lifecycle;

            let timmy = match lifecycle {
                Lifecycle::Perennial { duration } => {
                    let t = self.timers.get_mut(i).unwrap();
                    t.until_finish = duration;
                    *t
                }
                Lifecycle::Annual => self.timers.swap_remove(i),
            };

            match finish_timer(ss, timmy) {
                Ok(n) => ss.send_note(Note::Rude(n)),
                Err(e) => log::error!("error finishing timer {:#?}: {}", timmy, e),
            }
        }

        SessSendSubmit::Submit
    }
}
