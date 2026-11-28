//! Undo and redo by differences (F14-08).
//!
//! Each step keeps only the nodes, links and journal entries that changed, plus the project
//! counters, instead of a whole copy of the project. A review edit touches one object and one
//! journal entry, so 20 steps on a 20k network weigh kilobytes, not hundreds of megabytes.
//!
//! The terrain, the city context, the sources and the other project fields are not tracked: the
//! app edits only the review and the assigned model (D12), which never touch them.

use std::collections::VecDeque;

use crate::model::{EstadoProyecto, JournalEntry, Link, Node, Project};

/// Steps kept by [`History::default`].
pub const DEFAULT_LIMIT: usize = 20;

/// Positions of a vector that differ between two versions, with both lengths.
#[derive(Clone, Debug)]
struct VecChange<T> {
    old_len: usize,
    new_len: usize,
    old: Vec<(usize, T)>,
    new: Vec<(usize, T)>,
}

impl<T: Clone + PartialEq> VecChange<T> {
    fn between(old: &[T], new: &[T]) -> VecChange<T> {
        let mut change = VecChange {
            old_len: old.len(),
            new_len: new.len(),
            old: Vec::new(),
            new: Vec::new(),
        };
        for i in 0..old.len().max(new.len()) {
            let (before, after) = (old.get(i), new.get(i));
            if before != after {
                change.old.extend(before.map(|x| (i, x.clone())));
                change.new.extend(after.map(|x| (i, x.clone())));
            }
        }
        change
    }

    /// Whether some replaced item has a different index key (id, code, ends…), or the length
    /// changed: then the project indices must be rebuilt.
    fn moves_index(&self, same_key: impl Fn(&T, &T) -> bool) -> bool {
        self.old_len != self.new_len
            || self
                .old
                .iter()
                .zip(&self.new)
                .any(|((_, a), (_, b))| !same_key(a, b))
    }

    /// Writes one side of the change into `v`. Items are sorted by position, so those past the
    /// truncated end are appended in order.
    fn write(&self, v: &mut Vec<T>, side: Side) {
        let (len, items) = match side {
            Side::Before => (self.old_len, &self.old),
            Side::After => (self.new_len, &self.new),
        };
        v.truncate(len);
        for (i, item) in items {
            match v.get_mut(*i) {
                Some(slot) => *slot = item.clone(),
                None => v.push(item.clone()),
            }
        }
    }
}

/// Project fields that every edit may move.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Counters {
    revision: u64,
    estado: EstadoProyecto,
    next_slot: u32,
}

impl Counters {
    fn of(p: &Project) -> Counters {
        Counters {
            revision: p.revision,
            estado: p.estado,
            next_slot: p.next_slot,
        }
    }

    fn write(self, p: &mut Project) {
        p.revision = self.revision;
        p.estado = self.estado;
        p.next_slot = self.next_slot;
    }
}

#[derive(Clone, Copy)]
enum Side {
    Before,
    After,
}

/// One reversible edit.
#[derive(Clone, Debug)]
pub struct Change {
    nodes: VecChange<Node>,
    links: VecChange<Link>,
    journal: VecChange<JournalEntry>,
    before: Counters,
    after: Counters,
    reindex: bool,
}

/// The fields the project indices are built from (`indice.rs`).
fn same_node_key(a: &Node, b: &Node) -> bool {
    a.id == b.id && a.code == b.code && a.family == b.family
}

fn same_link_key(a: &Link, b: &Link) -> bool {
    a.id == b.id && a.code == b.code && a.family == b.family && a.from == b.from && a.to == b.to
}

impl Change {
    /// What changed from `before` to `after`.
    pub fn between(before: &Project, after: &Project) -> Change {
        let nodes = VecChange::between(&before.nodes, &after.nodes);
        let links = VecChange::between(&before.links, &after.links);
        let reindex = nodes.moves_index(same_node_key) || links.moves_index(same_link_key);
        Change {
            nodes,
            links,
            journal: VecChange::between(&before.journal, &after.journal),
            before: Counters::of(before),
            after: Counters::of(after),
            reindex,
        }
    }

    fn put(&self, p: &mut Project, side: Side) {
        self.nodes.write(&mut p.nodes, side);
        self.links.write(&mut p.links, side);
        self.journal.write(&mut p.journal, side);
        match side {
            Side::Before => self.before.write(p),
            Side::After => self.after.write(p),
        }
        if self.reindex {
            p.reindexar();
        }
    }

    /// Serialized size of what this step keeps: an estimate of its memory.
    pub fn estimated_bytes(&self) -> usize {
        fn bytes<T: serde::Serialize>(items: &[(usize, T)]) -> usize {
            items
                .iter()
                .map(|(_, x)| serde_json::to_vec(x).map_or(0, |v| v.len()))
                .sum()
        }
        bytes(&self.nodes.old)
            + bytes(&self.nodes.new)
            + bytes(&self.links.old)
            + bytes(&self.links.new)
            + bytes(&self.journal.old)
            + bytes(&self.journal.new)
    }
}

/// Undo and redo stacks of bounded length.
#[derive(Clone, Debug)]
pub struct History {
    undo: VecDeque<Change>,
    redo: Vec<Change>,
    limit: usize,
}

impl Default for History {
    fn default() -> History {
        History::with_limit(DEFAULT_LIMIT)
    }
}

impl History {
    pub fn with_limit(limit: usize) -> History {
        History {
            undo: VecDeque::new(),
            redo: Vec::new(),
            limit,
        }
    }

    /// Records the edit from `before` to `after`. A new edit discards what could be redone.
    pub fn record(&mut self, before: &Project, after: &Project) {
        self.undo.push_back(Change::between(before, after));
        if self.undo.len() > self.limit {
            self.undo.pop_front();
        }
        self.redo.clear();
    }

    /// Takes `p` back one step. `false` if there is nothing to undo.
    pub fn undo(&mut self, p: &mut Project) -> bool {
        let Some(change) = self.undo.pop_back() else {
            return false;
        };
        change.put(p, Side::Before);
        self.redo.push(change);
        true
    }

    /// Replays the last undone step. `false` if there is nothing to redo.
    pub fn redo(&mut self, p: &mut Project) -> bool {
        let Some(change) = self.redo.pop() else {
            return false;
        };
        change.put(p, Side::After);
        self.undo.push_back(change);
        true
    }

    pub fn undo_steps(&self) -> usize {
        self.undo.len()
    }

    pub fn redo_steps(&self) -> usize {
        self.redo.len()
    }

    /// Estimated memory of every kept step, undo and redo.
    pub fn estimated_bytes(&self) -> usize {
        self.undo
            .iter()
            .chain(&self.redo)
            .map(Change::estimated_bytes)
            .sum()
    }
}
