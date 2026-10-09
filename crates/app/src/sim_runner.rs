//! Drapes the student's garment on its own thread, so the window stays responsive even when a
//! slow computer simulates slower than real time. Play hands the thread a snapshot of the
//! project; the thread builds the fabric (meshing takes a moment) and the cloth, then steps
//! the solver. Reset drops the drape. The UI only reads the latest frame.

use arc_swap::ArcSwapOption;
use crossbeam_channel::{Receiver, Sender, unbounded};
use glam::Vec3;
use opendrape_core::Project;
use opendrape_drape::Stage;
pub use opendrape_drape::{DENSITY_KG_M2, DrapeNote, build_drape};
use opendrape_sim::Solver;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// One published simulation frame.
#[derive(Debug)]
pub struct SimFrame {
    /// Increases with every published frame, across drapes.
    pub seq: u64,
    /// Simulated seconds since Play.
    pub time: f64,
    pub positions: Vec<Vec3>,
    /// Shared until the topology changes (seams weld), so the renderer can skip re-uploads.
    pub triangles: Arc<Vec<[u32; 3]>>,
    /// Wall-clock milliseconds the last step took.
    pub step_ms: f64,
    /// What making this drape found.
    pub notes: Arc<Vec<DrapeNote>>,
}

enum Command {
    Play(Arc<Project>),
    Reset,
    Wake,
    Shutdown,
    /// Tests: treat the next step as having gone wrong.
    #[cfg(test)]
    Spoil,
}

pub struct SimRunner {
    tx: Sender<Command>,
    latest: Arc<ArcSwapOption<SimFrame>>,
    /// From Play until Reset (or until the drape goes wrong): the 3D view shows the drape.
    draping: Arc<AtomicBool>,
    playing: Arc<AtomicBool>,
    idle: Arc<AtomicBool>,
    went_wrong: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

/// Below this kinetic energy (J) for [`SETTLE_FRAMES`] frames, a welded drape counts as settled
/// and the simulation pauses itself, so a finished drape doesn't keep a laptop core busy.
const SETTLED_ENERGY: f64 = 1e-6;
const SETTLE_FRAMES: u32 = 60;

struct Status {
    latest: Arc<ArcSwapOption<SimFrame>>,
    draping: Arc<AtomicBool>,
    playing: Arc<AtomicBool>,
    idle: Arc<AtomicBool>,
    went_wrong: Arc<AtomicBool>,
}

impl SimRunner {
    /// Starts the simulation thread, with nothing to drape. `on_frame` runs after every
    /// published frame (the app asks for a repaint).
    pub fn start(stage: Arc<Stage>, on_frame: impl Fn() + Send + 'static) -> Self {
        let (tx, rx) = unbounded();
        let status = Status {
            latest: Arc::new(ArcSwapOption::empty()),
            draping: Arc::new(AtomicBool::new(false)),
            playing: Arc::new(AtomicBool::new(false)),
            idle: Arc::new(AtomicBool::new(false)),
            went_wrong: Arc::new(AtomicBool::new(false)),
        };
        let (latest, draping, playing, idle, went_wrong) = (
            status.latest.clone(),
            status.draping.clone(),
            status.playing.clone(),
            status.idle.clone(),
            status.went_wrong.clone(),
        );
        let thread = std::thread::Builder::new()
            .name("opendrape-sim".into())
            .spawn(move || run(&stage, &rx, &status, &on_frame))
            .expect("spawn the simulation thread");
        Self {
            tx,
            latest,
            draping,
            playing,
            idle,
            went_wrong,
            thread: Some(thread),
        }
    }
    /// Drapes `project`: the fabric is made on the simulation thread, then it runs.
    pub fn play(&self, project: Arc<Project>) {
        self.went_wrong.store(false, Ordering::Relaxed);
        self.draping.store(true, Ordering::Relaxed);
        self.playing.store(true, Ordering::Relaxed);
        let _ = self.tx.send(Command::Play(project));
    }
    /// Back to arranging: the drape is dropped.
    pub fn reset(&self) {
        self.draping.store(false, Ordering::Relaxed);
        self.playing.store(false, Ordering::Relaxed);
        let _ = self.tx.send(Command::Reset);
    }
    /// The latest frame of the drape; None while arranging (and while the fabric is made).
    pub fn latest(&self) -> Option<Arc<SimFrame>> {
        self.latest.load_full()
    }
    /// Between Play and Reset.
    pub fn is_draping(&self) -> bool {
        self.draping.load(Ordering::Relaxed)
    }
    pub fn is_playing(&self) -> bool {
        self.playing.load(Ordering::Relaxed)
    }
    /// True while the worker waits for a command: no step is in flight.
    pub fn is_idle(&self) -> bool {
        self.idle.load(Ordering::Acquire)
    }
    /// Pauses or resumes the drape.
    pub fn set_playing(&self, playing: bool) {
        self.playing.store(playing, Ordering::Relaxed);
        let _ = self.tx.send(Command::Wake);
    }
    /// Whether the last drape went wrong (a number stopped being finite) and was dropped.
    pub fn went_wrong(&self) -> bool {
        self.went_wrong.load(Ordering::Relaxed)
    }
    #[cfg(test)]
    fn spoil(&self) {
        let _ = self.tx.send(Command::Spoil);
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

/// A drape in progress.
struct Drape {
    solver: Solver,
    notes: Arc<Vec<DrapeNote>>,
    topology: u64,
    triangles: Arc<Vec<[u32; 3]>>,
}

impl Drape {
    fn frame(&mut self, seq: u64, step_ms: f64) -> SimFrame {
        let c = self.solver.cloth();
        if c.topology_version() != self.topology {
            self.topology = c.topology_version();
            self.triangles = Arc::new(c.triangles().to_vec());
        }
        SimFrame {
            seq,
            time: self.solver.time(),
            positions: c.positions().iter().map(|p| p.as_vec3()).collect(),
            triangles: self.triangles.clone(),
            step_ms,
            notes: self.notes.clone(),
        }
    }
    /// Every live particle is a finite number.
    fn is_finite(&self) -> bool {
        let c = self.solver.cloth();
        c.positions()
            .iter()
            .enumerate()
            .all(|(i, p)| !c.is_alive(i) || p.is_finite())
    }
}

fn run(stage: &Stage, rx: &Receiver<Command>, status: &Status, on_frame: &dyn Fn()) {
    let mut drape: Option<Drape> = None;
    let mut seq = 0;
    let mut next = Instant::now();
    let mut settled_frames = 0;
    let mut spoiled = false;
    loop {
        // Draping and playing: just look for a command. Otherwise sleep until one arrives.
        let busy = drape.is_some() && status.playing.load(Ordering::Relaxed);
        let cmd = if busy {
            rx.try_recv().ok()
        } else {
            status.idle.store(true, Ordering::Release);
            let cmd = rx.recv().unwrap_or(Command::Shutdown);
            status.idle.store(false, Ordering::Release);
            Some(cmd)
        };
        match cmd {
            Some(Command::Shutdown) => return,
            Some(Command::Play(project)) => {
                let (solver, notes) = build_drape(&project, stage);
                let c = solver.cloth();
                let mut d = Drape {
                    topology: c.topology_version(),
                    triangles: Arc::new(c.triangles().to_vec()),
                    solver,
                    notes: Arc::new(notes),
                };
                seq += 1;
                status.latest.store(Some(Arc::new(d.frame(seq, 0.0))));
                drape = Some(d);
                settled_frames = 0;
                on_frame();
                next = Instant::now();
                continue;
            }
            Some(Command::Reset) => {
                drape = None;
                status.latest.store(None);
                on_frame();
                continue;
            }
            Some(Command::Wake) => {
                next = Instant::now();
                continue;
            }
            #[cfg(test)]
            Some(Command::Spoil) => {
                spoiled = true;
                continue;
            }
            None => {}
        }
        let Some(d) = &mut drape else { continue };
        let started = Instant::now();
        let collider = stage.drape_collider();
        d.solver.step(Some(&collider));
        if std::mem::take(&mut spoiled) || !d.is_finite() {
            // Something pulled the cloth apart: drop the drape and say so.
            drape = None;
            status.latest.store(None);
            status.draping.store(false, Ordering::Relaxed);
            status.playing.store(false, Ordering::Relaxed);
            status.went_wrong.store(true, Ordering::Relaxed);
            on_frame();
            continue;
        }
        let cloth = d.solver.cloth();
        settled_frames = if !cloth.has_open_stitches() && cloth.kinetic_energy() < SETTLED_ENERGY {
            settled_frames + 1
        } else {
            0
        };
        if settled_frames >= SETTLE_FRAMES {
            status.playing.store(false, Ordering::Relaxed);
            settled_frames = 0;
        }
        seq += 1;
        let ms = started.elapsed().as_secs_f64() * 1000.0;
        status.latest.store(Some(Arc::new(d.frame(seq, ms))));
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
    use opendrape_core::{Half, Piece, PieceId, Placement, Point2, SeamSide};
    use opendrape_mesh::MeshParams;

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

    /// Two 200 × 300 mm panels sewn along one side, hanging in front of the form.
    fn two_panels() -> Arc<Project> {
        let mut pr = Project::new();
        let a = pr.add_piece(Piece::rectangle(
            PieceId(0),
            "A",
            Point2::new(0.0, 0.0),
            200.0,
            300.0,
        ));
        let b = pr.add_piece(Piece::rectangle(
            PieceId(0),
            "B",
            Point2::new(300.0, 0.0),
            200.0,
            300.0,
        ));
        pr.add_seam(
            SeamSide::new(a, Half::Drawn, 1, 1, true),
            SeamSide::new(b, Half::Drawn, 3, 1, false),
        );
        Arc::new(pr)
    }

    /// A small square lying flat 3 cm above the floor, beside the form: it drops and settles.
    fn on_the_floor() -> Arc<Project> {
        let mut pr = Project::new();
        let mut piece = Piece::rectangle(PieceId(0), "Square", Point2::new(0.0, 0.0), 100.0, 100.0);
        piece.placement = Some(Placement {
            position: [0.6, 0.03, 0.0],
            rotation: glam::DQuat::from_rotation_x(-std::f64::consts::FRAC_PI_2).to_array(),
            curve: None,
        });
        pr.add_piece(piece);
        Arc::new(pr)
    }

    #[test]
    fn play_builds_the_fabric_on_the_simulation_thread_and_runs_it() {
        let r = SimRunner::start(Stage::shared(), || {});
        assert!(
            r.latest().is_none() && !r.is_draping(),
            "nothing to drape yet"
        );
        let project = two_panels();
        let mesh = opendrape_mesh::build(&project, &MeshParams::default());
        r.play(project);
        assert!(r.is_draping());
        wait_for("simulated time to advance", || {
            r.latest().is_some_and(|f| f.time > 0.1)
        });
        assert_eq!(r.latest().unwrap().positions.len(), mesh.particles());
        assert!(r.latest().unwrap().notes.is_empty());
    }

    #[test]
    fn pausing_stops_time_and_reset_drops_the_drape() {
        let r = SimRunner::start(Stage::shared(), || {});
        r.play(two_panels());
        wait_for("time > 0.1", || r.latest().is_some_and(|f| f.time > 0.1));
        r.set_playing(false);
        wait_for("the worker to go idle", || r.is_idle());
        let t = r.latest().unwrap().time;
        std::thread::sleep(Duration::from_millis(150));
        assert_eq!(r.latest().unwrap().time, t, "paused");
        r.reset();
        wait_for("the drape to go", || r.latest().is_none());
        assert!(!r.is_draping());
    }

    #[test]
    fn a_settled_drape_pauses_itself() {
        let r = SimRunner::start(Stage::shared(), || {});
        r.play(on_the_floor());
        wait_for("auto-pause once settled", || !r.is_playing() && r.is_idle());
        let f = r.latest().expect("still showing the drape");
        assert!(
            f.positions.iter().all(|p| p.y < 0.02 && p.y > 0.0),
            "on the floor"
        );
        std::thread::sleep(Duration::from_millis(150));
        assert_eq!(r.latest().unwrap().time, f.time);
    }

    #[test]
    fn a_drape_that_goes_wrong_is_dropped_and_reported() {
        let r = SimRunner::start(Stage::shared(), || {});
        r.play(two_panels());
        wait_for("the drape", || r.latest().is_some());
        r.spoil();
        wait_for("the drape to be dropped", || r.went_wrong());
        assert!(r.latest().is_none() && !r.is_draping() && !r.is_playing());
        r.play(two_panels());
        assert!(!r.went_wrong(), "a new Play starts afresh");
    }

    #[test]
    fn play_with_nothing_drawn_is_harmless() {
        let r = SimRunner::start(Stage::shared(), || {});
        r.play(Arc::new(Project::new()));
        wait_for("an empty drape that settles at once", || {
            r.latest().is_some() && !r.is_playing() && r.is_idle()
        });
        assert!(r.latest().unwrap().positions.is_empty());
        assert!(!r.went_wrong());
    }

    #[test]
    fn dropping_the_runner_stops_its_thread() {
        let r = SimRunner::start(Stage::shared(), || {});
        r.play(two_panels());
        wait_for("running", || r.latest().is_some_and(|f| f.time > 0.0));
        let start = Instant::now();
        drop(r);
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "thread joined promptly"
        );
    }
}
