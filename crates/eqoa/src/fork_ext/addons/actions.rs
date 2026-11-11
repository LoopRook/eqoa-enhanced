// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Action and window dispatch for `eqoa.actions` and `eqoa.windows`.
use std::cell::RefCell;
use std::collections::VecDeque;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClientAction {
    Assist,
    OpenSummary,
}

thread_local! {
    static ACTION_QUEUE: RefCell<VecDeque<ClientAction>> = const { RefCell::new(VecDeque::new()) };
}

pub fn queue_assist() -> bool {
    ACTION_QUEUE.with(|q| {
        let mut q = q.borrow_mut();
        if q.len() < 16 {
            q.push_back(ClientAction::Assist);
            true
        } else {
            false
        }
    })
}

pub fn queue_open_summary() -> bool {
    ACTION_QUEUE.with(|q| {
        let mut q = q.borrow_mut();
        if q.len() < 16 {
            q.push_back(ClientAction::OpenSummary);
            true
        } else {
            false
        }
    })
}

pub fn drain_actions() -> Vec<ClientAction> {
    ACTION_QUEUE.with(|q| {
        let mut q = q.borrow_mut();
        q.drain(..).collect()
    })
}

pub fn clear_actions() {
    ACTION_QUEUE.with(|q| q.borrow_mut().clear());
}

pub struct ActionRequests {
    pub assist: bool,
    pub summary: bool,
}

pub fn request_assist() {
    queue_assist();
}

pub fn request_summary() {
    queue_open_summary();
}

pub fn take_action_requests() -> ActionRequests {
    let mut assist = false;
    let mut summary = false;
    for a in drain_actions() {
        match a {
            ClientAction::Assist => assist = true,
            ClientAction::OpenSummary => summary = true,
        }
    }
    ActionRequests { assist, summary }
}
