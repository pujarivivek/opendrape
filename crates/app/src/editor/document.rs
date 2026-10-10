//! The open project and its undo history.

use opendrape_core::{ModelError, Pin, Project};
use std::path::PathBuf;

/// Undo steps kept; older ones are dropped.
pub const UNDO_LIMIT: usize = 200;

/// How the pins' numbers changed when a change added or took away pins. A pin is known by its
/// place in [`Project::pins`], so whatever holds one by number (the selection, a pin being
/// dragged, a menu open on a pin) must follow it when pins before it go, and let go when its
/// own pin does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PinShift {
    /// For each number a pin had before, the number it has now; None when that pin is gone.
    now: Vec<Option<usize>>,
}

impl PinShift {
    /// Pins `before` and `after` a change, matched in order by what they are: the longest run
    /// of pins that are the same in both. A pin the change also altered can't be told from a
    /// gone one, and counts as gone (so what held it lets go rather than hold another).
    pub fn between(before: &[Pin], after: &[Pin]) -> Self {
        let (n, m) = (before.len(), after.len());
        if n == m {
            return Self {
                now: (0..n).map(Some).collect(),
            };
        }
        // The usual table of longest common runs, from the ends back to the starts.
        let mut common = vec![vec![0u16; m + 1]; n + 1];
        for i in (0..n).rev() {
            for j in (0..m).rev() {
                common[i][j] = if before[i] == after[j] {
                    common[i + 1][j + 1] + 1
                } else {
                    common[i + 1][j].max(common[i][j + 1])
                };
            }
        }
        let mut now = vec![None; n];
        let (mut i, mut j) = (0, 0);
        while i < n && j < m {
            if before[i] == after[j] {
                now[i] = Some(j);
                i += 1;
                j += 1;
            } else if common[i + 1][j] >= common[i][j + 1] {
                i += 1;
            } else {
                j += 1;
            }
        }
        Self { now }
    }

    /// Every pin is gone: the project was replaced.
    pub fn none_kept() -> Self {
        Self { now: Vec::new() }
    }

    /// The number pin `k` has now; None when it is gone.
    pub fn index(&self, k: usize) -> Option<usize> {
        self.now.get(k).copied().flatten()
    }
}

/// The project being edited. Undo keeps whole snapshots of the project (a pattern is small), so
/// there is no reverse-edit code to get wrong.
pub struct Document {
    project: Project,
    undo: Vec<Project>,
    redo: Vec<Project>,
    /// The project as it was when the current drag began.
    gesture: Option<Project>,
    /// The project as last saved or opened, to tell whether there are unsaved changes.
    saved: Project,
    /// Why the last [`Self::edit`] or [`Self::gesture_edit`] was refused; None if it was not.
    refused: Option<ModelError>,
    /// How pins were renumbered by changes since [`Self::take_pin_shifts`] last asked.
    pin_shifts: Vec<PinShift>,
    /// Where the project was last saved or opened from.
    pub path: Option<PathBuf>,
}

impl Default for Document {
    fn default() -> Self {
        Self::new(Project::new(), None)
    }
}

impl Document {
    pub fn new(project: Project, path: Option<PathBuf>) -> Self {
        Self {
            saved: project.clone(),
            project,
            undo: Vec::new(),
            redo: Vec::new(),
            gesture: None,
            refused: None,
            pin_shifts: Vec::new(),
            path,
        }
    }
    /// Work brought back from a recovery copy: unsaved, and Save writes it to `path` (the file
    /// it came from) when there is one.
    pub fn recovered(project: Project, path: Option<PathBuf>) -> Self {
        let mut doc = Self::new(project, path);
        doc.saved = Project::new();
        doc
    }
    pub fn project(&self) -> &Project {
        &self.project
    }
    /// Changes the project as one undo step. What the change leaves unusable goes with it (a
    /// seam side 1 mm long or less, a pin off its piece: see [`Project::drop_broken`]). A change
    /// that leaves the project as it was adds no step. A change that leaves the project invalid
    /// (see [`Project::check`]) is refused: the project is left as it was and no step is added.
    /// Ask [`Self::last_change_refused`] to tell a refusal from a change that simply did nothing:
    /// the closure's result is returned either way.
    pub fn edit<R>(&mut self, f: impl FnOnce(&mut Project) -> R) -> R {
        self.end_gesture();
        let before = self.project.clone();
        let result = f(&mut self.project);
        self.project.drop_broken();
        self.refused = self.project.check().err();
        if self.refused.is_some() {
            // Never keep a project that could not be saved and opened again.
            self.project = before;
        } else {
            self.note_pins(&before.pins);
            if self.project != before {
                self.push_undo(before);
            }
        }
        result
    }
    /// The last [`Self::edit`] or [`Self::gesture_edit`] was refused because it would have left
    /// the project invalid (too many points, say), and so was undone at once.
    pub fn last_change_refused(&self) -> bool {
        self.refused.is_some()
    }
    /// What was wrong with the project the last change would have made, when it was refused.
    pub fn last_refusal(&self) -> Option<&ModelError> {
        self.refused.as_ref()
    }
    /// Starts a drag: everything changed with [`Self::gesture_edit`] until [`Self::end_gesture`]
    /// is one undo step.
    pub fn begin_gesture(&mut self) {
        if self.gesture.is_none() {
            self.gesture = Some(self.project.clone());
        }
    }
    /// Changes the project as part of the current drag (starting one if needed). What it leaves
    /// unusable goes, and an invalid result is refused, as in [`Self::edit`].
    pub fn gesture_edit<R>(&mut self, f: impl FnOnce(&mut Project) -> R) -> R {
        self.begin_gesture();
        let before = self.project.clone();
        let result = f(&mut self.project);
        self.project.drop_broken();
        self.refused = self.project.check().err();
        if self.refused.is_some() {
            self.project = before;
        } else {
            self.note_pins(&before.pins);
        }
        result
    }
    pub fn end_gesture(&mut self) {
        if let Some(before) = self.gesture.take()
            && before != self.project
        {
            self.push_undo(before);
        }
    }
    /// Records how the pins were renumbered, if the project's pins are not as many as `before`.
    fn note_pins(&mut self, before: &[Pin]) {
        if before.len() != self.project.pins.len() {
            self.pin_shifts
                .push(PinShift::between(before, &self.project.pins));
        }
    }
    /// How pins were renumbered by the changes since this was last asked, oldest first.
    pub fn take_pin_shifts(&mut self) -> Vec<PinShift> {
        std::mem::take(&mut self.pin_shifts)
    }
    fn push_undo(&mut self, before: Project) {
        self.undo.push(before);
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
    }
    pub fn undo(&mut self) -> bool {
        self.end_gesture();
        let Some(previous) = self.undo.pop() else {
            return false;
        };
        let now = std::mem::replace(&mut self.project, previous);
        self.note_pins(&now.pins);
        self.redo.push(now);
        true
    }
    pub fn redo(&mut self) -> bool {
        self.end_gesture();
        let Some(next) = self.redo.pop() else {
            return false;
        };
        let was = std::mem::replace(&mut self.project, next);
        self.note_pins(&was.pins);
        self.undo.push(was);
        true
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    /// The project differs from the version last saved or opened.
    pub fn is_dirty(&self) -> bool {
        self.project != self.saved
    }
    pub fn mark_saved(&mut self, path: PathBuf) {
        self.saved = self.project.clone();
        self.path = Some(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendrape_core::{Piece, PieceId, Pin, Point2};

    fn rect() -> Piece {
        Piece::rectangle(PieceId(0), "R", Point2::new(0.0, 0.0), 100.0, 50.0)
    }

    #[test]
    fn edits_undo_and_redo() {
        let mut doc = Document::default();
        assert!(!doc.can_undo() && !doc.can_redo());
        let id = doc.edit(|p| p.add_piece(rect()));
        assert_eq!(doc.project().pieces.len(), 1);
        assert!(doc.undo());
        assert!(doc.project().pieces.is_empty());
        assert!(!doc.undo(), "nothing left to undo");
        assert!(doc.redo());
        assert!(doc.project().piece(id).is_some());
        assert!(!doc.redo());
    }

    #[test]
    fn recovered_work_is_unsaved_and_belongs_to_the_file_it_came_from() {
        let mut project = Project::new();
        project.add_piece(rect());
        let from = PathBuf::from("/work/skirt.odp");
        let doc = Document::recovered(project.clone(), Some(from.clone()));
        assert!(doc.is_dirty());
        assert_eq!(doc.project(), &project);
        assert_eq!(doc.path, Some(from.clone()));
        assert!(!doc.can_undo(), "no history comes back with it");
        let mut doc = doc;
        doc.mark_saved(from);
        assert!(!doc.is_dirty(), "saving makes it clean like any other work");
        assert!(Document::recovered(project.clone(), None).is_dirty());
        assert!(!Document::new(project, None).is_dirty());
    }

    #[test]
    fn a_change_that_changes_nothing_adds_no_step() {
        let mut doc = Document::default();
        doc.edit(|p| p.remove_piece(PieceId(7)));
        assert!(!doc.can_undo());
    }

    #[test]
    fn a_new_edit_clears_redo() {
        let mut doc = Document::default();
        doc.edit(|p| p.add_piece(rect()));
        doc.undo();
        doc.edit(|p| p.add_piece(rect()));
        assert!(!doc.can_redo());
    }

    #[test]
    fn a_whole_drag_is_one_step() {
        let mut doc = Document::default();
        let id = doc.edit(|p| p.add_piece(rect()));
        doc.begin_gesture();
        for k in 1..=10 {
            doc.gesture_edit(|p| {
                p.piece_mut(id)
                    .unwrap()
                    .move_vertex(0, Point2::new(-f64::from(k), 0.0))
            });
        }
        doc.end_gesture();
        assert_eq!(
            doc.project().piece(id).unwrap().vertices[0].pos,
            Point2::new(-10.0, 0.0)
        );
        assert!(doc.undo());
        assert_eq!(
            doc.project().piece(id).unwrap().vertices[0].pos,
            Point2::new(0.0, 0.0)
        );
        assert!(doc.undo(), "then the piece itself");
        assert!(!doc.can_undo());
    }

    #[test]
    fn a_drag_that_moved_nothing_adds_no_step() {
        let mut doc = Document::default();
        doc.begin_gesture();
        doc.end_gesture();
        assert!(!doc.can_undo());
    }

    #[test]
    fn history_keeps_the_last_200_steps() {
        let mut doc = Document::default();
        for _ in 0..UNDO_LIMIT + 50 {
            doc.edit(|p| p.add_piece(rect()));
        }
        let mut undone = 0;
        while doc.undo() {
            undone += 1;
        }
        assert_eq!(undone, UNDO_LIMIT);
        assert_eq!(doc.project().pieces.len(), 50);
    }

    #[test]
    fn dirty_follows_the_saved_version() {
        let mut doc = Document::default();
        assert!(!doc.is_dirty());
        doc.edit(|p| p.add_piece(rect()));
        assert!(doc.is_dirty());
        doc.undo();
        assert!(!doc.is_dirty(), "undone back to the saved version");
        doc.redo();
        doc.mark_saved(PathBuf::from("skirt.odp"));
        assert!(!doc.is_dirty());
        assert_eq!(doc.path, Some(PathBuf::from("skirt.odp")));
        doc.undo();
        assert!(doc.is_dirty());
    }

    #[test]
    fn last_change_refused_tells_a_refusal_from_a_no_op() {
        let mut doc = Document::default();
        assert!(!doc.last_change_refused());
        let id = doc.edit(|p| p.add_piece(rect()));
        assert!(!doc.last_change_refused());
        doc.edit(|p| p.remove_piece(PieceId(99)));
        assert!(!doc.last_change_refused(), "a change that did nothing");
        doc.edit(|p| p.piece_mut(id).unwrap().vertices.truncate(2));
        assert!(doc.last_change_refused());
        assert!(doc.last_refusal().is_some(), "and why");
        doc.edit(|p| p.piece_mut(id).unwrap().name = "Back".into());
        assert!(!doc.last_change_refused(), "the next change starts afresh");
        assert_eq!(doc.last_refusal(), None);

        doc.begin_gesture();
        doc.gesture_edit(|p| p.piece_mut(id).unwrap().vertices.truncate(2));
        assert!(doc.last_change_refused());
        doc.gesture_edit(|p| {
            p.piece_mut(id)
                .unwrap()
                .move_vertex(0, Point2::new(1.0, 1.0))
        });
        assert!(!doc.last_change_refused());
        doc.end_gesture();
    }

    #[test]
    fn the_501st_piece_is_refused() {
        let mut doc = Document::default();
        for _ in 0..opendrape_core::MAX_PIECES {
            doc.edit(|p| p.add_piece(rect()));
            assert!(!doc.last_change_refused());
        }
        doc.edit(|p| p.add_piece(rect()));
        assert!(doc.last_change_refused());
        assert_eq!(doc.project().pieces.len(), opendrape_core::MAX_PIECES);
    }

    #[test]
    fn a_change_that_breaks_the_project_is_refused() {
        let mut doc = Document::default();
        let id = doc.edit(|p| p.add_piece(rect()));
        doc.edit(|p| {
            p.piece_mut(id)
                .unwrap()
                .move_vertex(0, Point2::new(2e6, 0.0))
        });
        assert_eq!(
            doc.project().piece(id).unwrap().vertices[0].pos,
            Point2::new(0.0, 0.0)
        );
        doc.begin_gesture();
        doc.gesture_edit(|p| p.piece_mut(id).unwrap().vertices.truncate(2));
        doc.end_gesture();
        assert_eq!(doc.project().piece(id).unwrap().len(), 4);
        assert!(doc.undo(), "only adding the piece was a step");
        assert!(!doc.can_undo());
    }

    #[test]
    fn a_drag_that_leaves_a_seam_side_too_short_deletes_the_seam_in_the_same_step() {
        use opendrape_core::{Half, OutlinePos, SeamSide};
        let mut doc = Document::default();
        let (a, b) = doc.edit(|p| (p.add_piece(rect()), p.add_piece(rect())));
        // A's bottom edge (100 mm) from 10 to 20 mm, to B's left edge.
        let part = SeamSide {
            from: OutlinePos::new(0, 0.1),
            to: OutlinePos::new(0, 0.2),
            ..SeamSide::edges(a, Half::Drawn, 0, 0, true)
        };
        doc.edit(|p| p.add_seam(part, SeamSide::edges(b, Half::Drawn, 3, 3, false)));
        let steps = |doc: &mut Document| {
            let mut n = 0;
            while doc.undo() {
                n += 1;
            }
            for _ in 0..n {
                doc.redo();
            }
            n
        };
        assert_eq!(steps(&mut doc), 2);
        // Pull A's bottom-right corner in to 5 mm: the side would be 0.5 mm long.
        doc.gesture_edit(|p| {
            p.piece_mut(a)
                .unwrap()
                .move_vertex(1, Point2::new(5.0, 0.0))
        });
        doc.end_gesture();
        assert!(!doc.last_change_refused(), "the edit is made");
        assert!(doc.project().seams.is_empty(), "and the seam goes with it");
        assert_eq!(steps(&mut doc), 3, "in the same step");
        doc.undo();
        assert_eq!(doc.project().seams.len(), 1, "undo brings both back");
    }

    fn pin(x: f64) -> Pin {
        Pin {
            shape: PieceId(1),
            half: opendrape_core::Half::Drawn,
            at: Point2::new(x, 10.0),
            target: [0.0, 1.0, 0.5],
        }
    }

    /// Where each of pins `0..n` went, as a list.
    fn went(shift: &PinShift, n: usize) -> Vec<Option<usize>> {
        (0..n).map(|k| shift.index(k)).collect()
    }

    #[test]
    fn pins_after_a_removed_one_move_down_and_it_is_gone() {
        let (a, b, c) = (pin(10.0), pin(20.0), pin(30.0));
        assert_eq!(
            went(&PinShift::between(&[a, b, c], &[a, c]), 4),
            vec![Some(0), None, Some(1), None],
            "the middle one went; nothing is numbered past the end"
        );
        assert_eq!(
            went(&PinShift::between(&[a, b, c], &[b, c]), 3),
            vec![None, Some(0), Some(1)]
        );
        assert_eq!(
            went(&PinShift::between(&[a, b, c], &[a, b]), 3),
            vec![Some(0), Some(1), None]
        );
        // Put back (an undo): the ones after it move up.
        assert_eq!(
            went(&PinShift::between(&[b, c], &[a, b, c]), 2),
            vec![Some(1), Some(2)]
        );
        // Two pins alike: whichever went, the others keep their order.
        assert_eq!(
            went(&PinShift::between(&[a, b, a], &[b, a]), 3),
            vec![None, Some(0), Some(1)]
        );
        // A pin the change also altered counts as gone, not as another pin.
        let moved = Pin {
            target: [0.5, 1.0, 0.5],
            ..b
        };
        assert_eq!(
            went(&PinShift::between(&[a, b, c], &[moved, c]), 3),
            vec![None, None, Some(1)]
        );
        // As many pins as before: the same pins, whatever was done to them.
        assert_eq!(
            went(&PinShift::between(&[a, b], &[moved, b]), 2),
            vec![Some(0), Some(1)]
        );
        assert_eq!(went(&PinShift::none_kept(), 2), vec![None, None]);
    }

    #[test]
    fn every_way_the_pins_change_is_recorded_as_a_shift() {
        let mut doc = Document::default();
        let id = doc.edit(|p| {
            let id = p.add_piece(rect());
            p.pins = vec![pin(10.0), pin(20.0), pin(30.0)];
            id
        });
        assert_eq!(doc.take_pin_shifts().len(), 1, "three pins came");
        assert!(doc.take_pin_shifts().is_empty(), "asked once");
        // A removal.
        doc.edit(|p| {
            p.pins.remove(0);
        });
        let shifts = doc.take_pin_shifts();
        assert_eq!(went(&shifts[0], 3), vec![None, Some(0), Some(1)]);
        // Undo brings it back, redo takes it away again.
        doc.undo();
        let shifts = doc.take_pin_shifts();
        assert_eq!(went(&shifts[0], 2), vec![Some(1), Some(2)]);
        doc.redo();
        let shifts = doc.take_pin_shifts();
        assert_eq!(went(&shifts[0], 3), vec![None, Some(0), Some(1)]);
        // A pin dragged, or a change that leaves them as many: no shift.
        doc.gesture_edit(|p| p.pins[0].target = [0.1, 1.0, 0.5]);
        doc.end_gesture();
        doc.edit(|p| p.pins[1].target = [0.2, 1.0, 0.5]);
        assert!(doc.take_pin_shifts().is_empty());
        // A change a gesture makes counts as it is made.
        doc.gesture_edit(|p| {
            p.pins.pop();
        });
        doc.end_gesture();
        assert_eq!(went(&doc.take_pin_shifts()[0], 2), vec![Some(0), None]);
        // A pin lost with its piece, and a change that is refused (too many pins): no shift.
        doc.edit(|p| p.remove_piece(id));
        assert!(doc.project().pins.is_empty());
        assert_eq!(went(&doc.take_pin_shifts()[0], 1), vec![None]);
        doc.undo();
        doc.take_pin_shifts();
        doc.edit(|p| p.pins = vec![pin(10.0); 501]);
        assert!(doc.last_change_refused());
        assert!(doc.take_pin_shifts().is_empty());
    }
}
