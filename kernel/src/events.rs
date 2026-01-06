use crossbeam_queue::ArrayQueue;
use pc_keyboard::KeyCode;
use spin::Lazy;

use crate::serial_println;

const EVENT_QUEUE_SIZE: usize = 1024;

static EVENT_QUEUE: Lazy<ArrayQueue<Event>> = Lazy::new(|| ArrayQueue::new(EVENT_QUEUE_SIZE));

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    KeyboardEvent(KeyboardEvent),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyboardEvent {
    KeyPressed(KeyCode),
    KeyReleased(KeyCode),
    SingleShot(KeyCode),
}

pub fn push_event(event: Event) {
    if let Err(_) = EVENT_QUEUE.push(event) {
        serial_println!("[WARNING] Event queue full, dropping event: {:?}", event);
    }
}

pub fn pop_event() -> Option<Event> {
    EVENT_QUEUE.pop()
}
pub fn has_events() -> bool {
    !EVENT_QUEUE.is_empty()
}
