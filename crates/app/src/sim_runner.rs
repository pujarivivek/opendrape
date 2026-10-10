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
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// One published simulation frame.
#[derive(Debug)]
pub struct SimFrame {
    /// Which Play made it (see [`SimRunner::latest`]).
    pub drape: u64,
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
    /// Drape this project; its frames are numbered `drape`.
    Play(Arc<Project>, u64),
    Reset,
    Wake,
    Shutdown,
}

pub struct SimRunner {
    tx: Sender<Command>,
    latest: Arc<ArcSwapOption<SimFrame>>,
    /// From Play until Reset (or until the drape goes wrong): the 3D view shows the drape.
    draping: Arc<AtomicBool>,
    playing: Arc<AtomicBool>,
    idle: Arc<AtomicBool>,
    went_wrong: Arc<AtomicBool>,
    /// The number of the Play whose frames the 3D view shows. Play and Reset both move it on, so
    /// a frame an earlier drape publishes after Reset (the thread may be in the middle of a
    /// step) is never shown.
    shown: Arc<AtomicU64>,
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
    shown: Arc<AtomicU64>,
}

impl Status {
    /// Drape number `drape` went wrong (a number stopped being finite, or making it panicked):
    /// drop it, stop, and say so. A drape the student has already moved on from (Reset, or a
    /// newer Play) is dropped silently.
    fn fail(&self, drape: u64) {
        if self.shown.load(Ordering::Acquire) != drape {
            return;
        }
        self.latest.store(None);
        self.draping.store(false, Ordering::Relaxed);
        self.playing.store(false, Ordering::Relaxed);
        self.went_wrong.store(true, Ordering::Release);
    }
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
            shown: Arc::new(AtomicU64::new(0)),
        };
        let (latest, draping, playing, idle, went_wrong, shown) = (
            status.latest.clone(),
            status.draping.clone(),
            status.playing.clone(),
            status.idle.clone(),
            status.went_wrong.clone(),
            status.shown.clone(),
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
            shown,
            thread: Some(thread),
        }
    }
    /// Drapes `project`: the fabric is made on the simulation thread, then it runs.
    pub fn play(&self, project: Arc<Project>) {
        let drape = self.shown.fetch_add(1, Ordering::AcqRel) + 1;
        self.went_wrong.store(false, Ordering::Release);
        self.draping.store(true, Ordering::Relaxed);
        self.playing.store(true, Ordering::Relaxed);
        let _ = self.tx.send(Command::Play(project, drape));
    }
    /// Back to arranging: the drape is dropped, and its last frame is gone at once.
    pub fn reset(&self) {
        self.shown.fetch_add(1, Ordering::AcqRel);
        self.draping.store(false, Ordering::Relaxed);
        self.playing.store(false, Ordering::Relaxed);
        self.latest.store(None);
        let _ = self.tx.send(Command::Reset);
    }
    /// The latest frame of the drape since the last Play; None while arranging (and while the
    /// fabric is made). A frame the thread was still working on when Reset (or a newer Play)
    /// came is not shown.
    pub fn latest(&self) -> Option<Arc<SimFrame>> {
        let shown = self.shown.load(Ordering::Acquire);
        self.latest.load_full().filter(|f| f.drape == shown)
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
    /// Whether the last drape went wrong (a number stopped being finite, or making it
    /// panicked) and was dropped.
    pub fn went_wrong(&self) -> bool {
        self.went_wrong.load(Ordering::Acquire)
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
    /// The Play it came from.
    number: u64,
    solver: Solver,
    notes: Arc<Vec<DrapeNote>>,
    topology: u64,
    triangles: Arc<Vec<[u32; 3]>>,
}

impl Drape {
    fn new(number: u64, solver: Solver, notes: Vec<DrapeNote>) -> Self {
        let c = solver.cloth();
        Self {
            number,
            topology: c.topology_version(),
            triangles: Arc::new(c.triangles().to_vec()),
            solver,
            notes: Arc::new(notes),
        }
    }

    /// The frame to publish, or None if any live particle is not a finite number as the
    /// renderer will get it (single precision: a double too big for one counts too). Every
    /// frame, the first included, goes through here before anyone sees it.
    fn frame(&mut self, seq: u64, step_ms: f64) -> Option<SimFrame> {
        let c = self.solver.cloth();
        let positions: Vec<Vec3> = c.positions().iter().map(|p| p.as_vec3()).collect();
        if positions
            .iter()
            .enumerate()
            .any(|(i, p)| c.is_alive(i) && !p.is_finite())
        {
            return None;
        }
        if c.topology_version() != self.topology {
            self.topology = c.topology_version();
            self.triangles = Arc::new(c.triangles().to_vec());
        }
        Some(SimFrame {
            drape: self.number,
            seq,
            time: self.solver.time(),
            positions,
            triangles: self.triangles.clone(),
            step_ms,
            notes: self.notes.clone(),
        })
    }
}

/// `work()`, or None if it panicked: a pattern that sends the mesher or the solver somewhere it
/// should never go must end the drape with a message, not the simulation thread without a word;
/// and the 3D view's fabric (`arrange::scene`), made on the thread that draws the window, must
/// not end the program.
pub(crate) fn guarded<T>(work: impl FnOnce() -> T) -> Option<T> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(work)).ok()
}

/// Makes drape `number` with `make` (its first frame too) and publishes that frame; the drape
/// goes on to be stepped. If `make` panics, or gives no drape because its first frame is not
/// made of numbers the renderer can take, the drape went wrong: it is reported (see
/// [`Status::fail`]) and there is nothing to step.
fn start_drape(
    status: &Status,
    number: u64,
    make: impl FnOnce() -> Option<(Drape, SimFrame)>,
) -> Option<Drape> {
    match guarded(make).flatten() {
        Some((drape, first)) => {
            status.latest.store(Some(Arc::new(first)));
            Some(drape)
        }
        None => {
            status.fail(number);
            None
        }
    }
}

fn run(stage: &Stage, rx: &Receiver<Command>, status: &Status, on_frame: &dyn Fn()) {
    let mut drape: Option<Drape> = None;
    let mut seq = 0;
    let mut next = Instant::now();
    let mut settled_frames = 0;
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
            Some(Command::Play(project, number)) => {
                // The old drape goes first, so its memory is free for the new one.
                drop(drape.take());
                seq += 1;
                // Making the fabric, and the first frame, are checked like every later frame.
                drape = start_drape(status, number, || {
                    let (solver, notes) = build_drape(&project, stage);
                    let mut d = Drape::new(number, solver, notes);
                    let first = d.frame(seq, 0.0);
                    first.map(|f| (d, f))
                });
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
            None => {}
        }
        let Some(d) = &mut drape else { continue };
        let started = Instant::now();
        let collider = stage.drape_collider();
        let stepped = guarded(|| d.solver.step(Some(&collider))).is_some();
        seq += 1;
        let ms = started.elapsed().as_secs_f64() * 1000.0;
        let frame = if stepped { d.frame(seq, ms) } else { None };
        let Some(frame) = frame else {
            // Something pulled the cloth apart (or the solver panicked): drop the drape and say so.
            status.fail(d.number);
            drape = None;
            on_frame();
            continue;
        };
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
        status.latest.store(Some(Arc::new(frame)));
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
            SeamSide::edges(a, Half::Drawn, 1, 1, true),
            SeamSide::edges(b, Half::Drawn, 3, 3, false),
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

    /// A project with one piece carrying this placement, built directly: `Document` would
    /// refuse a placement like these, but the runner takes any project it is handed.
    fn project_placed(position: [f64; 3], rotation: [f64; 4]) -> Arc<Project> {
        let mut pr = Project::new();
        let mut piece = Piece::rectangle(PieceId(0), "Bad", Point2::new(0.0, 0.0), 100.0, 100.0);
        piece.placement = Some(Placement {
            position,
            rotation,
            curve: None,
        });
        pr.add_piece(piece);
        Arc::new(pr)
    }

    /// Placements whose cloth is not made of numbers the renderer can take: not a number, too
    /// big for single precision (finite as a double), and a rotation that is not a number.
    fn unusable_placements() -> Vec<Arc<Project>> {
        let up = Placement::NO_ROTATION;
        vec![
            project_placed([f64::NAN, 1.0, 0.5], up),
            project_placed([1e39, 1.0, 0.5], up),
            project_placed([0.0, 1.0, 0.5], [f64::NAN, 0.0, 0.0, 1.0]),
        ]
    }

    #[test]
    fn a_frame_is_only_made_from_numbers_the_renderer_can_take() {
        let stage = Stage::shared();
        for project in unusable_placements() {
            let (solver, notes) = build_drape(&project, &stage);
            let mut drape = Drape::new(1, solver, notes);
            assert!(drape.frame(1, 0.0).is_none(), "the first frame");
        }
        let (solver, notes) = build_drape(&two_panels(), &stage);
        let mut drape = Drape::new(7, solver, notes);
        let frame = drape.frame(1, 0.0).expect("an ordinary drape");
        assert_eq!(frame.drape, 7);
        assert!(frame.positions.iter().all(|p| p.is_finite()));
        // Later frames go through the same check.
        let collider = stage.drape_collider();
        for _ in 0..3 {
            drape.solver.step(Some(&collider));
            assert!(drape.frame(2, 1.0).is_some());
        }
    }

    #[test]
    fn a_drape_that_goes_wrong_is_dropped_and_reported_before_anything_is_shown() {
        for project in unusable_placements() {
            let r = SimRunner::start(Stage::shared(), || {});
            r.play(project);
            wait_for("the drape to be dropped", || {
                assert!(r.latest().is_none(), "a frame of it was shown");
                r.went_wrong()
            });
            assert!(r.latest().is_none() && !r.is_draping() && !r.is_playing());
            r.play(two_panels());
            assert!(!r.went_wrong(), "a new Play starts afresh");
            wait_for("a good drape after it", || {
                r.latest().is_some_and(|f| f.time > 0.05)
            });
            assert!(!r.went_wrong());
        }
    }

    /// The state the thread shares with the window, as `SimRunner::start` makes it.
    fn status() -> Status {
        Status {
            latest: Arc::new(ArcSwapOption::empty()),
            draping: Arc::new(AtomicBool::new(false)),
            playing: Arc::new(AtomicBool::new(false)),
            idle: Arc::new(AtomicBool::new(false)),
            went_wrong: Arc::new(AtomicBool::new(false)),
            shown: Arc::new(AtomicU64::new(0)),
        }
    }

    #[test]
    fn a_panic_while_making_the_fabric_is_a_drape_gone_wrong_not_a_dead_thread() {
        // Whatever sends the mesher or the solver somewhere it should never go, the work is
        // done inside `guarded`, and a panic there is reported like any other failed drape.
        let status = status();
        status.shown.store(3, Ordering::Release); // the third Play
        status.draping.store(true, Ordering::Relaxed);
        status.playing.store(true, Ordering::Relaxed);
        let made = start_drape(&status, 3, || -> Option<(Drape, SimFrame)> {
            panic!("the mesher gave up")
        });
        assert!(made.is_none(), "there is nothing to step");
        assert!(status.went_wrong.load(Ordering::Acquire));
        assert!(status.latest.load().is_none());
        assert!(!status.draping.load(Ordering::Relaxed));
        assert!(!status.playing.load(Ordering::Relaxed));

        // A drape the student has already moved on from (a newer Play) goes quietly.
        status.went_wrong.store(false, Ordering::Release);
        status.shown.store(4, Ordering::Release);
        let made = start_drape(&status, 3, || -> Option<(Drape, SimFrame)> {
            panic!("late")
        });
        assert!(made.is_none());
        assert!(
            !status.went_wrong.load(Ordering::Acquire),
            "only the drape being shown can go wrong"
        );

        // A drape that is made is kept, and its first frame is published.
        let (solver, notes) = build_drape(&two_panels(), &Stage::shared());
        let mut drape = Drape::new(4, solver, notes);
        let first = drape.frame(1, 0.0).expect("an ordinary first frame");
        let made = start_drape(&status, 4, move || Some((drape, first)));
        assert!(made.is_some());
        assert_eq!(status.latest.load().as_ref().map(|f| f.drape), Some(4));
        assert!(!status.went_wrong.load(Ordering::Acquire));

        // A first frame the renderer can't take is a drape gone wrong, too.
        let made = start_drape(&status, 4, || None);
        assert!(made.is_none());
        assert!(status.went_wrong.load(Ordering::Acquire));
    }

    #[test]
    fn a_cut_out_of_no_points_does_not_stop_the_drape() {
        // `Document` refuses such a cut-out, but the runner takes any project it is handed: the
        // mesher skips the cut-out and the piece drapes.
        let mut pr = Project::new();
        let mut piece = Piece::rectangle(PieceId(0), "Bad", Point2::new(0.0, 0.0), 100.0, 100.0);
        piece.lines = vec![opendrape_core::InternalLine {
            vertices: vec![],
            edges: vec![],
            closed: true,
            kind: opendrape_core::LineKind::Cutout,
        }];
        pr.add_piece(piece);
        let r = SimRunner::start(Stage::shared(), || {});
        r.play(Arc::new(pr));
        wait_for("a drape", || r.latest().is_some_and(|f| f.time > 0.05));
        assert!(!r.went_wrong());
    }

    #[test]
    fn guarded_work_that_panics_gives_none() {
        assert_eq!(guarded(|| 3), Some(3));
        assert_eq!(guarded(|| -> u8 { panic!("boom") }), None);
    }

    #[test]
    fn reset_takes_the_drape_away_at_once_and_a_later_frame_of_it_is_never_shown() {
        let r = SimRunner::start(Stage::shared(), || {});
        r.play(two_panels());
        wait_for("a frame", || r.latest().is_some());
        let last = r.latest().expect("a frame");
        // Straight after Reset, with the thread asleep between steps or in the middle of one:
        r.reset();
        assert!(r.latest().is_none(), "no stale drape after Reset");
        assert!(
            r.latest.load().is_none(),
            "the published frame is cleared by Reset itself"
        );
        // A frame the thread was still working on, published after Reset, is not shown.
        r.latest.store(Some(last));
        assert!(r.latest().is_none(), "a late frame of the dropped drape");
        // Reset then Play: nothing of the first drape shows before the second has a frame.
        r.play(two_panels());
        r.reset();
        r.play(two_panels());
        assert!(
            r.latest().is_none_or(|f| f.drape == 5),
            "only the latest drape's frames"
        );
        wait_for("the second drape", || {
            r.latest().is_some_and(|f| f.time > 0.05)
        });
        assert_eq!(
            r.latest().unwrap().drape,
            5,
            "Play, Reset, Play, Reset, Play"
        );
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
