//! Runs the cloth simulation on its own thread so the window stays responsive even when a
//! slow computer simulates slower than real time. The UI only reads the latest frame.

use arc_swap::ArcSwap;
use crossbeam_channel::{Receiver, Sender, unbounded};
use glam::Vec3;
use opendrape_testkit::garments::{Garment, Scene};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// One published simulation frame.
#[derive(Debug)]
pub struct SimFrame {
    /// Increases with every published frame, across resets.
    pub seq: u64,
    /// Simulated seconds since the scene started.
    pub time: f64,
    pub positions: Vec<Vec3>,
    /// Shared until the topology changes (seams weld), so the renderer can skip re-uploads.
    pub triangles: Arc<Vec<[u32; 3]>>,
    /// Wall-clock milliseconds the last step took.
    pub step_ms: f64,
}

enum Command {
    Reset(Garment),
    Wake,
    Shutdown,
}

pub struct SimRunner {
    tx: Sender<Command>,
    latest: Arc<ArcSwap<SimFrame>>,
    playing: Arc<AtomicBool>,
    idle: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

/// Below this kinetic energy (J) for [`SETTLE_FRAMES`] frames, a welded drape counts as settled
/// and the simulation pauses itself, so a finished drape doesn't keep a laptop core busy.
const SETTLED_ENERGY: f64 = 1e-6;
const SETTLE_FRAMES: u32 = 60;

impl SimRunner {
    pub fn start(garment: Garment, playing: bool, on_frame: impl Fn() + Send + 'static) -> Self {
        let (tx, rx) = unbounded();
        let scene = Scene::new(garment);
        let mut publisher = Publisher::new(&scene, 0);
        let latest = Arc::new(ArcSwap::from_pointee(publisher.frame(&scene, 0.0)));
        let playing = Arc::new(AtomicBool::new(playing));
        let idle = Arc::new(AtomicBool::new(false));
        let thread = {
            let (latest, playing, idle) = (latest.clone(), playing.clone(), idle.clone());
            std::thread::Builder::new()
                .name("opendrape-sim".into())
                .spawn(move || run(scene, publisher, &rx, &latest, &playing, &idle, &on_frame))
                .expect("spawn the simulation thread")
        };
        Self {
            tx,
            latest,
            playing,
            idle,
            thread: Some(thread),
        }
    }
    pub fn latest(&self) -> Arc<SimFrame> {
        self.latest.load_full()
    }
    pub fn is_playing(&self) -> bool {
        self.playing.load(Ordering::Relaxed)
    }
    /// True while the worker is paused and waiting: no step is in flight.
    pub fn is_idle(&self) -> bool {
        self.idle.load(Ordering::Acquire)
    }
    pub fn set_playing(&self, playing: bool) {
        self.playing.store(playing, Ordering::Relaxed);
        let _ = self.tx.send(Command::Wake);
    }
    pub fn reset(&self, garment: Garment) {
        let _ = self.tx.send(Command::Reset(garment));
    }
}

impl Drop for SimRunner {
    fn drop(&mut self) {
        let _ = self.tx.send(Command::Shutdown);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

struct Publisher {
    seq: u64,
    topology: u64,
    triangles: Arc<Vec<[u32; 3]>>,
}

impl Publisher {
    fn new(scene: &Scene, seq: u64) -> Self {
        let c = scene.solver.cloth();
        Self {
            seq,
            topology: c.topology_version(),
            triangles: Arc::new(c.triangles().to_vec()),
        }
    }
    fn frame(&mut self, scene: &Scene, step_ms: f64) -> SimFrame {
        let c = scene.solver.cloth();
        if c.topology_version() != self.topology {
            self.topology = c.topology_version();
            self.triangles = Arc::new(c.triangles().to_vec());
        }
        self.seq += 1;
        SimFrame {
            seq: self.seq,
            time: scene.solver.time(),
            positions: c.positions().iter().map(|p| p.as_vec3()).collect(),
            triangles: self.triangles.clone(),
            step_ms,
        }
    }
}

fn run(
    mut scene: Scene,
    mut publisher: Publisher,
    rx: &Receiver<Command>,
    latest: &ArcSwap<SimFrame>,
    playing: &AtomicBool,
    idle: &AtomicBool,
    on_frame: &dyn Fn(),
) {
    let mut next = Instant::now();
    let mut settled_frames = 0;
    loop {
        // Paused: sleep until a command arrives. Playing: just look.
        let cmd = if playing.load(Ordering::Relaxed) {
            rx.try_recv().ok()
        } else {
            idle.store(true, Ordering::Release);
            let cmd = rx.recv().unwrap_or(Command::Shutdown);
            idle.store(false, Ordering::Release);
            Some(cmd)
        };
        match cmd {
            Some(Command::Shutdown) => return,
            Some(Command::Reset(g)) => {
                scene = Scene::new(g);
                settled_frames = 0;
                publisher = Publisher::new(&scene, publisher.seq);
                latest.store(Arc::new(publisher.frame(&scene, 0.0)));
                on_frame();
                next = Instant::now();
                continue;
            }
            Some(Command::Wake) => {
                next = Instant::now();
                continue;
            }
            None => {}
        }
        let started = Instant::now();
        scene.step();
        let cloth = scene.solver.cloth();
        settled_frames = if !cloth.has_open_stitches() && cloth.kinetic_energy() < SETTLED_ENERGY {
            settled_frames + 1
        } else {
            0
        };
        if settled_frames >= SETTLE_FRAMES {
            playing.store(false, Ordering::Relaxed);
            settled_frames = 0;
        }
        latest.store(Arc::new(
            publisher.frame(&scene, started.elapsed().as_secs_f64() * 1000.0),
        ));
        on_frame();
        // Real time at most; a slow computer simply runs slower.
        next += Duration::from_secs_f64(opendrape_sim::FRAME_DT);
        let now = Instant::now();
        if next > now {
            std::thread::sleep(next - now);
        } else {
            next = now;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendrape_testkit::garments::{BODICE_PARTICLES, SKIRT_PARTICLES};

    fn wait_for(what: &str, mut cond: impl FnMut() -> bool) {
        let start = Instant::now();
        while !cond() {
            assert!(
                start.elapsed() < Duration::from_secs(20),
                "timed out waiting for {what}"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn the_simulation_runs_without_the_window_drawing() {
        let r = SimRunner::start(Garment::Skirt, true, || {});
        assert_eq!(r.latest().positions.len(), SKIRT_PARTICLES);
        wait_for("simulated time to advance", || r.latest().time > 0.1);
    }

    #[test]
    fn pausing_stops_time_and_reset_rewinds_it() {
        let r = SimRunner::start(Garment::Skirt, true, || {});
        wait_for("time > 0.1", || r.latest().time > 0.1);
        r.set_playing(false);
        wait_for("the worker to go idle", || r.is_idle());
        let t = r.latest().time;
        std::thread::sleep(Duration::from_millis(150));
        assert_eq!(r.latest().time, t, "paused");
        r.reset(Garment::Skirt);
        wait_for("reset to t = 0", || r.latest().time == 0.0);
    }

    #[test]
    fn a_settled_drape_pauses_itself() {
        // Simulating a garment that no longer moves would keep a laptop core busy forever.
        let r = SimRunner::start(Garment::BodiceProxy, true, || {});
        wait_for("auto-pause once settled", || !r.is_playing() && r.is_idle());
        let t = r.latest().time;
        std::thread::sleep(Duration::from_millis(150));
        assert_eq!(r.latest().time, t);
    }

    #[test]
    fn rapid_resets_end_on_the_last_choice() {
        let r = SimRunner::start(Garment::Skirt, true, || {});
        for g in [Garment::BodiceProxy, Garment::Skirt, Garment::BodiceProxy] {
            r.reset(g);
        }
        wait_for("the fitted tube", || {
            r.latest().positions.len() == BODICE_PARTICLES
        });
    }

    #[test]
    fn dropping_the_runner_stops_its_thread() {
        let r = SimRunner::start(Garment::Skirt, true, || {});
        wait_for("running", || r.latest().time > 0.0);
        let start = Instant::now();
        drop(r);
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "thread joined promptly"
        );
    }
}
