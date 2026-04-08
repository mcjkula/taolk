use std::cell::Cell;
use std::io::Cursor;
use std::time::{Duration, Instant};

use rodio::{DeviceSinkBuilder, MixerDeviceSink};

use crate::config::Notifications;

const DM_BYTES: &[u8] = include_bytes!("audio/dm.ogg");
const AMBIENT_BYTES: &[u8] = include_bytes!("audio/ambient.ogg");
const MENTION_BYTES: &[u8] = include_bytes!("audio/mention.ogg");

const THROTTLE: Duration = Duration::from_millis(250);

#[derive(Copy, Clone)]
pub enum Sound {
    Dm,
    Ambient,
    Mention,
}

pub struct Audio {
    sink: Option<MixerDeviceSink>,
    cfg: Notifications,
    last_play: Cell<Instant>,
}

impl Audio {
    pub fn from_config(cfg: &Notifications) -> Self {
        let sink = DeviceSinkBuilder::open_default_sink().ok().map(|mut s| {
            s.log_on_drop(false);
            s
        });
        Self {
            sink,
            cfg: cfg.clone(),
            last_play: Cell::new(Instant::now() - Duration::from_secs(60)),
        }
    }

    pub fn play(&self, sound: Sound) {
        if !self.cfg.enabled {
            return;
        }
        let allowed = match sound {
            Sound::Dm => self.cfg.dm,
            Sound::Ambient => self.cfg.ambient,
            Sound::Mention => self.cfg.mention,
        };
        if !allowed {
            return;
        }
        let Some(sink) = &self.sink else { return };

        let now = Instant::now();
        if now.duration_since(self.last_play.get()) < THROTTLE {
            return;
        }
        self.last_play.set(now);

        let bytes = match sound {
            Sound::Dm => DM_BYTES,
            Sound::Ambient => AMBIENT_BYTES,
            Sound::Mention => MENTION_BYTES,
        };
        let pct = f32::from(self.cfg.volume.min(100)) / 100.0;
        let gain = pct * pct;

        if let Ok(player) = rodio::play(sink.mixer(), Cursor::new(bytes)) {
            player.set_volume(gain);
            player.detach();
        }
    }
}
