use crate::{
    git::BranchSummary,
    jira::{BoardSummary, IssueSummary},
};
use crossterm::event::{Event as CrosstermEvent, EventStream, KeyCode};
use futures::{future::FutureExt, StreamExt};
use tokio::sync::mpsc;
pub enum Event {
    KeyEvent(KeyCode),
    IssuesUpdated(Vec<IssueSummary>),
    BoardsUpdated(Vec<BoardSummary>),
    BranchesUpdated(Vec<BranchSummary>),
}
pub type EventsTx = mpsc::UnboundedSender<Event>;
pub type EventsRx = mpsc::UnboundedReceiver<Event>;

pub fn subscribe_to_key_events(tx: EventsTx) {
    let mut reader = EventStream::new();

    tokio::spawn(async move {
        loop {
            let event = reader.next().fuse();
            tokio::select! {
                maybe_event = event => {
                    match maybe_event {
                        Some(Ok(event)) => {
                            if let CrosstermEvent::Key(input) = event {
                                let _ = tx.send(Event::KeyEvent(input.code));

                            }
                        },
                        _ => break,
                    }
                }
            }
        }
    });
}
