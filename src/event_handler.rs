use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::time::{Instant, Duration};
use std::ptr;
use std::sync::mpsc::Sender;
use std::cell::Cell;

use core_foundation::runloop::{kCFRunLoopCommonModes, CFRunLoopSource, CFRunLoop};
use core_foundation::mach_port::CFMachPortCreateRunLoopSource;
use core_foundation::base::TCFType;
use core_graphics::event::{
    CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement, CGEventTapProxy, CGEventType,
    CGEventFlags, CGEvent, EventField, CGEventTap
};
use mouse_rs::Mouse;

pub const RCMD_TAP_DURATION_MS: u128 = 200;
pub const RIGHT_COMMAND_KEY_CODE: i64 = 54;
pub const LEFT_SHIFT_KEY_CODE: i64 = 56;
pub const ESCAPE_KEY_CODE: i64 = 53;

#[derive(Debug)]
pub enum GlobalEvent {
    ShowGrid(Option<eframe::egui::Pos2>),
}

pub struct EventTapSharedState {
    pub event_tx: Sender<GlobalEvent>,
    pub app_is_visible: Arc<AtomicBool>,
    pub eframe_hide_requested_by_listener: Arc<AtomicBool>,
    pub lshift_key_is_pressed: Arc<AtomicBool>,
}

fn is_modifier_key_code(key_code: i64) -> bool {
    matches!(key_code, 54 | 55 | 56 | 57 | 58 | 59 | 60 | 61 | 62 | 63)
}

pub fn global_event_listener_thread(shared_state: EventTapSharedState) {
    println!("Global event listener started");
    let rcmd_press_time: Cell<Option<Instant>> = Cell::new(None);
    let current_run_loop = CFRunLoop::get_current();

    let callback_closure = move |_proxy: CGEventTapProxy, event_type: CGEventType, event: &CGEvent| -> Option<CGEvent> {
        if shared_state.app_is_visible.load(AtomicOrdering::SeqCst) {
            match event_type {
                CGEventType::KeyDown => {
                    let key_code = event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE);
                    if key_code == ESCAPE_KEY_CODE {
                        println!("Escape pressed, hiding app");
                        shared_state.eframe_hide_requested_by_listener.store(true, AtomicOrdering::SeqCst);
                        return None; // Consume the event
                    }
                }
                _ => {}
            }
        }

        match event_type {
            CGEventType::FlagsChanged => {
                let flags = event.get_flags();
                let key_code = event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE);

                if key_code == RIGHT_COMMAND_KEY_CODE {
                    if flags.contains(CGEventFlags::CGEventFlagCommand) { // RCMD Pressed
                        if rcmd_press_time.get().is_none() {
                            rcmd_press_time.set(Some(Instant::now()));
                        }
                    } else { // RCMD Released
                        if let Some(press_time_instant) = rcmd_press_time.take() {
                            if press_time_instant.elapsed() < Duration::from_millis(RCMD_TAP_DURATION_MS as u64) {
                                // Tap detected
                                if !shared_state.app_is_visible.load(AtomicOrdering::SeqCst) {
                                    println!("Right cmd tap detected, showing grid");
                                    let mouse = Mouse::new();
                                    let cursor_pos = match mouse.get_position() {
                                        Ok(point) => Some(eframe::egui::pos2(point.x as f32, point.y as f32)),
                                        Err(_) => None,
                                    };
                                    let _ = shared_state.event_tx.send(GlobalEvent::ShowGrid(cursor_pos));
                                } else {
                                     println!("Right cmd tap ignored, app already visible");
                                }
                            }
                        }
                    }
                } else if key_code == LEFT_SHIFT_KEY_CODE {
                    if flags.contains(CGEventFlags::CGEventFlagShift) {
                        if !shared_state.lshift_key_is_pressed.load(AtomicOrdering::SeqCst) {
                             shared_state.lshift_key_is_pressed.store(true, AtomicOrdering::SeqCst);
                             println!("Left shift pressed");
                        }
                    } else {
                        if shared_state.lshift_key_is_pressed.load(AtomicOrdering::SeqCst) {
                            shared_state.lshift_key_is_pressed.store(false, AtomicOrdering::SeqCst);
                            println!("Left shift released");
                        }
                    }
                } else {
                    // If any other key changes flags (e.g., holding RCMD and pressing another modifier), reset RCMD timer
                     if rcmd_press_time.get().is_some() { 
                        rcmd_press_time.set(None);
                     }
                }
            }
            CGEventType::KeyDown => {
                // If RCMD is held and a non-modifier key is pressed, it's not a tap, so reset timer.
                let key_code = event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE);
                if rcmd_press_time.get().is_some() && !is_modifier_key_code(key_code) && key_code != RIGHT_COMMAND_KEY_CODE {
                    rcmd_press_time.set(None);
                }
            }
            _ => {}
        }
        Some(event.clone()) // Pass through the event
    };

    let tap_result = CGEventTap::new(
        CGEventTapLocation::HID,        // HID system events
        CGEventTapPlacement::HeadInsertEventTap, // Insert at head of event stream
        CGEventTapOptions::ListenOnly,  // Listen only, don't modify
        vec![CGEventType::KeyDown, CGEventType::KeyUp, CGEventType::FlagsChanged], // Events to tap
        callback_closure,
    );

    match tap_result {
        Ok(tap) => {
            unsafe {
                // Create a run loop source from the event tap
                let mach_port_ref = tap.mach_port.as_concrete_TypeRef();
                let source = CFMachPortCreateRunLoopSource(ptr::null_mut(), mach_port_ref, 0);
                if source.is_null() {
                    eprintln!("Failed to create run loop source");
                    return;
                }
                let cf_run_loop_source = CFRunLoopSource::wrap_under_get_rule(source);
                // Add the source to the current run loop
                current_run_loop.add_source(&cf_run_loop_source, kCFRunLoopCommonModes);
                // Enable the event tap
                tap.enable();
            }
            println!("Event tap enabled");
            CFRunLoop::run_current(); // Start the run loop
            println!("Event loop exited"); // Should not happen in normal operation
        }
        Err(e) => {
            eprintln!("Failed to create event tap: {:?}", e);
        }
    }
} 