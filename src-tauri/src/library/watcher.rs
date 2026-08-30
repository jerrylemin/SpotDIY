use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;

use notify::event::{CreateKind, ModifyKind, RemoveKind, RenameMode};
use notify::{Event, EventKind, RecursiveMode, Watcher};

use crate::domain::LibraryFolderId;

pub const DEBOUNCE_WINDOW: Duration = Duration::from_millis(450);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WatchAction {
    Create(PathBuf),
    Modify(PathBuf),
    Remove(PathBuf),
    Rename { from: PathBuf, to: PathBuf },
    Reconcile,
    WatcherFailure,
}

pub type WatchActionHandler =
    Arc<dyn Fn(LibraryFolderId, Vec<WatchAction>) + Send + Sync + 'static>;

struct WatcherRegistration {
    _watcher: notify::RecommendedWatcher,
    stop_sender: mpsc::Sender<()>,
}

pub struct WatcherRegistry {
    watchers: Mutex<HashMap<LibraryFolderId, WatcherRegistration>>,
}

impl WatcherRegistry {
    pub fn new() -> Self {
        Self {
            watchers: Mutex::new(HashMap::new()),
        }
    }

    pub fn register(
        &self,
        folder_id: LibraryFolderId,
        root: &Path,
        handler: WatchActionHandler,
    ) -> Result<(), notify::Error> {
        let (sender, receiver) = mpsc::channel::<notify::Result<Event>>();
        let callback_sender = sender.clone();
        let mut watcher = notify::recommended_watcher(move |event| {
            let _ = callback_sender.send(event);
        })?;
        watcher.watch(root, RecursiveMode::Recursive)?;

        let (stop_sender, stop_receiver) = mpsc::channel();
        let callback_handler = handler.clone();
        thread::Builder::new()
            .name(format!("spotdiy-library-watcher-{folder_id}"))
            .spawn(move || debounce_events(folder_id, receiver, stop_receiver, callback_handler))
            .map_err(|error| {
                let message = error.to_string();
                notify::Error::generic(&message)
            })?;

        let mut watchers = self
            .watchers
            .lock()
            .map_err(|_| notify::Error::generic("watcher registry lock is poisoned"))?;
        if let Some(previous) = watchers.insert(
            folder_id,
            WatcherRegistration {
                _watcher: watcher,
                stop_sender,
            },
        ) {
            let _ = previous.stop_sender.send(());
        }
        Ok(())
    }

    pub fn unregister(&self, folder_id: LibraryFolderId) {
        if let Ok(mut watchers) = self.watchers.lock() {
            if let Some(previous) = watchers.remove(&folder_id) {
                let _ = previous.stop_sender.send(());
            }
        }
    }

    pub fn is_registered(&self, folder_id: LibraryFolderId) -> bool {
        self.watchers
            .lock()
            .is_ok_and(|watchers| watchers.contains_key(&folder_id))
    }
}

impl Default for WatcherRegistry {
    fn default() -> Self {
        Self::new()
    }
}

fn debounce_events(
    folder_id: LibraryFolderId,
    receiver: mpsc::Receiver<notify::Result<Event>>,
    stop_receiver: mpsc::Receiver<()>,
    handler: WatchActionHandler,
) {
    loop {
        if should_stop(&stop_receiver) {
            return;
        }
        let first = match receiver.recv() {
            Ok(first) => first,
            Err(_) => {
                if !should_stop(&stop_receiver) {
                    handler(folder_id, vec![WatchAction::WatcherFailure]);
                }
                return;
            }
        };
        let mut batch = vec![first];
        while let Ok(next) = receiver.recv_timeout(DEBOUNCE_WINDOW) {
            batch.push(next);
        }
        if should_stop(&stop_receiver) {
            return;
        }
        let actions = coalesce_events(&batch);
        if !actions.is_empty() {
            handler(folder_id, actions);
        }
    }
}

fn should_stop(receiver: &mpsc::Receiver<()>) -> bool {
    matches!(
        receiver.try_recv(),
        Ok(()) | Err(mpsc::TryRecvError::Disconnected)
    )
}

pub fn coalesce_events(events: &[notify::Result<Event>]) -> Vec<WatchAction> {
    let mut actions = Vec::new();
    let mut pending_from = None;
    for event in events {
        let Ok(event) = event else {
            flush_pending_rename(&mut actions, &mut pending_from);
            push_unique(&mut actions, WatchAction::Reconcile);
            push_unique(&mut actions, WatchAction::WatcherFailure);
            continue;
        };
        if event.need_rescan() {
            flush_pending_rename(&mut actions, &mut pending_from);
            push_unique(&mut actions, WatchAction::Reconcile);
            continue;
        }
        match event.kind {
            EventKind::Create(kind) => {
                flush_pending_rename(&mut actions, &mut pending_from);
                if matches!(
                    kind,
                    CreateKind::Any | CreateKind::File | CreateKind::Folder
                ) {
                    if event.paths.is_empty() {
                        push_unique(&mut actions, WatchAction::Reconcile);
                    } else {
                        for path in &event.paths {
                            push_path_action(&mut actions, WatchAction::Create(path.clone()));
                        }
                    }
                } else {
                    push_unique(&mut actions, WatchAction::Reconcile);
                }
            }
            EventKind::Remove(kind) => {
                flush_pending_rename(&mut actions, &mut pending_from);
                if matches!(
                    kind,
                    RemoveKind::Any | RemoveKind::File | RemoveKind::Folder
                ) {
                    if event.paths.is_empty() {
                        push_unique(&mut actions, WatchAction::Reconcile);
                    } else {
                        for path in &event.paths {
                            push_path_action(&mut actions, WatchAction::Remove(path.clone()));
                        }
                    }
                } else {
                    push_unique(&mut actions, WatchAction::Reconcile);
                }
            }
            EventKind::Modify(ModifyKind::Name(mode)) => match mode {
                RenameMode::Both if event.paths.len() == 2 => {
                    flush_pending_rename(&mut actions, &mut pending_from);
                    push_unique(
                        &mut actions,
                        WatchAction::Rename {
                            from: event.paths[0].clone(),
                            to: event.paths[1].clone(),
                        },
                    );
                }
                RenameMode::From if event.paths.len() == 1 => {
                    flush_pending_rename(&mut actions, &mut pending_from);
                    pending_from = event.paths.first().cloned();
                }
                RenameMode::To if event.paths.len() == 1 => {
                    if let Some(from) = pending_from.take() {
                        push_unique(
                            &mut actions,
                            WatchAction::Rename {
                                from,
                                to: event.paths[0].clone(),
                            },
                        );
                    } else {
                        push_unique(&mut actions, WatchAction::Reconcile);
                    }
                }
                _ => {
                    flush_pending_rename(&mut actions, &mut pending_from);
                    push_unique(&mut actions, WatchAction::Reconcile);
                }
            },
            EventKind::Modify(ModifyKind::Any | ModifyKind::Data(_) | ModifyKind::Metadata(_)) => {
                flush_pending_rename(&mut actions, &mut pending_from);
                if event.paths.is_empty() {
                    push_unique(&mut actions, WatchAction::Reconcile);
                } else {
                    for path in &event.paths {
                        push_path_action(&mut actions, WatchAction::Modify(path.clone()));
                    }
                }
            }
            EventKind::Modify(ModifyKind::Other) | EventKind::Any | EventKind::Other => {
                flush_pending_rename(&mut actions, &mut pending_from);
                push_unique(&mut actions, WatchAction::Reconcile);
            }
            EventKind::Access(_) => flush_pending_rename(&mut actions, &mut pending_from),
        }
    }
    flush_pending_rename(&mut actions, &mut pending_from);
    actions
}

fn flush_pending_rename(actions: &mut Vec<WatchAction>, pending_from: &mut Option<PathBuf>) {
    if pending_from.take().is_some() {
        push_unique(actions, WatchAction::Reconcile);
    }
}

fn push_path_action(actions: &mut Vec<WatchAction>, action: WatchAction) {
    if actions
        .iter()
        .any(|existing| same_path_action(existing, &action))
    {
        return;
    }
    actions.push(action);
}

fn same_path_action(first: &WatchAction, second: &WatchAction) -> bool {
    match (first, second) {
        (WatchAction::Create(first), WatchAction::Create(second))
        | (WatchAction::Modify(first), WatchAction::Modify(second))
        | (WatchAction::Remove(first), WatchAction::Remove(second)) => first == second,
        _ => false,
    }
}

fn push_unique(actions: &mut Vec<WatchAction>, action: WatchAction) {
    if !actions.contains(&action) {
        actions.push(action);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::Flag;

    fn event(kind: EventKind, path: &str) -> notify::Result<Event> {
        Ok(Event {
            kind,
            paths: vec![PathBuf::from(path)],
            attrs: Default::default(),
        })
    }

    #[test]
    fn coalesces_duplicate_modifications_and_pairs_rename_events() {
        let events = vec![
            event(EventKind::Modify(ModifyKind::Any), "C:\\Music\\song.flac"),
            event(EventKind::Modify(ModifyKind::Any), "C:\\Music\\song.flac"),
            event(
                EventKind::Modify(ModifyKind::Name(RenameMode::From)),
                "C:\\Music\\old.flac",
            ),
            event(
                EventKind::Modify(ModifyKind::Name(RenameMode::To)),
                "C:\\Music\\new.flac",
            ),
        ];

        assert_eq!(
            coalesce_events(&events),
            vec![
                WatchAction::Modify(PathBuf::from("C:\\Music\\song.flac")),
                WatchAction::Rename {
                    from: PathBuf::from("C:\\Music\\old.flac"),
                    to: PathBuf::from("C:\\Music\\new.flac"),
                },
            ]
        );
    }

    #[test]
    fn an_unpaired_rename_requests_reconciliation() {
        let events = vec![event(
            EventKind::Modify(ModifyKind::Name(RenameMode::From)),
            "C:\\Music\\old.flac",
        )];
        assert_eq!(coalesce_events(&events), vec![WatchAction::Reconcile]);
    }

    #[test]
    fn malformed_or_interrupted_rename_sequences_request_reconciliation() {
        let malformed = Ok(Event {
            kind: EventKind::Modify(ModifyKind::Name(RenameMode::Both)),
            paths: vec![
                PathBuf::from("C:\\Music\\old.flac"),
                PathBuf::from("C:\\Music\\new.flac"),
                PathBuf::from("C:\\Music\\extra.flac"),
            ],
            attrs: Default::default(),
        });
        assert_eq!(coalesce_events(&[malformed]), vec![WatchAction::Reconcile]);

        let interrupted = vec![
            event(
                EventKind::Modify(ModifyKind::Name(RenameMode::From)),
                "C:\\Music\\old.flac",
            ),
            event(
                EventKind::Access(notify::event::AccessKind::Any),
                "C:\\Music\\other.flac",
            ),
            event(
                EventKind::Modify(ModifyKind::Name(RenameMode::To)),
                "C:\\Music\\new.flac",
            ),
        ];
        assert_eq!(coalesce_events(&interrupted), vec![WatchAction::Reconcile]);
    }

    #[test]
    fn notify_rescan_and_unknown_modifications_request_reconciliation() {
        let mut rescan = Event::new(EventKind::Other);
        rescan.attrs.set_flag(Flag::Rescan);
        rescan.paths.push(PathBuf::from("C:\\Music"));
        assert_eq!(coalesce_events(&[Ok(rescan)]), vec![WatchAction::Reconcile]);

        let unknown = event(
            EventKind::Modify(ModifyKind::Other),
            "C:\\Music\\unknown.flac",
        );
        assert_eq!(coalesce_events(&[unknown]), vec![WatchAction::Reconcile]);
    }

    #[test]
    fn watcher_errors_request_reconciliation_and_recovery() {
        let error: notify::Result<Event> = Err(notify::Error::generic("backend failure"));
        let actions = coalesce_events(&[error]);
        assert!(actions.contains(&WatchAction::Reconcile));
        assert!(actions.contains(&WatchAction::WatcherFailure));
    }
}
