//! The open project and its undo history.

use opendrape_core::{ModelError, Project};
use std::path::PathBuf;

/// Undo steps kept; older ones are dropped.
pub const UNDO_LIMIT: usize = 200;

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
    /// Changes the project as one undo step. A change that leaves the project as it was adds
    /// no step. A change that leaves the project invalid (see [`Project::check`]) is refused:
    /// the project is left as it was and no step is added. Ask [`Self::last_change_refused`]
    /// to tell a refusal from a change that simply did nothing: the closure's result is
    /// returned either way.
    pub fn edit<R>(&mut self, f: impl FnOnce(&mut Project) -> R) -> R {
        self.end_gesture();
        let before = self.project.clone();
        let result = f(&mut self.project);
        self.refused = self.project.check().err();
        if self.refused.is_some() {
            // Never keep a project that could not be saved and opened again.
            self.project = before;
        } else if self.project != before {
            self.push_undo(before);
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
    /// Changes the project as part of the current drag (starting one if needed). An invalid
    /// result is refused, as in [`Self::edit`]: the project is left as it was.
    pub fn gesture_edit<R>(&mut self, f: impl FnOnce(&mut Project) -> R) -> R {
        self.begin_gesture();
        let before = self.project.clone();
        let result = f(&mut self.project);
        self.refused = self.project.check().err();
        if self.refused.is_some() {
            self.project = before;
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
        self.redo
            .push(std::mem::replace(&mut self.project, previous));
        true
    }
    pub fn redo(&mut self) -> bool {
        self.end_gesture();
        let Some(next) = self.redo.pop() else {
            return false;
        };
        self.undo.push(std::mem::replace(&mut self.project, next));
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
    use opendrape_core::{Piece, PieceId, Point2};

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
}
