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

pub const RCMD_TAP_DURATION_MS: u128 = 100;
pub const RCMD_DOUBLE_TAP_MAX_DELAY_MS: u128 = 200; // Max delay between releases for a double tap
pub const RIGHT_COMMAND_KEY_CODE: i64 = 54;
pub const LEFT_SHIFT_KEY_CODE: i64 = 56;
pub const ESCAPE_KEY_CODE: i64 = 53;

#[derive(Debug)]
pub enum GlobalEvent {
    PotentialSingleRCmdTap { tap_time: Instant, cursor_pos: Option<eframe::egui::Pos2> },
    RCmdDoubleTap,
    CancelPendingRCmdTap
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
    let rcmd_press_start_time: Cell<Option<Instant>> = Cell::new(None);
    let first_tap_release_time_for_double_tap: Cell<Option<Instant>> = Cell::new(None);
    let current_run_loop = CFRunLoop::get_current();

    let callback_closure = move |_proxy: CGEventTapProxy, event_type: CGEventType, event: &CGEvent| -> Option<CGEvent> {
        if shared_state.app_is_visible.load(AtomicOrdering::SeqCst) {
            match event_type {
                CGEventType::KeyDown => {
                    let key_code = event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE);
                    if key_code == ESCAPE_KEY_CODE {
                        println!("Escape pressed, hiding app");
                        shared_state.eframe_hide_requested_by_listener.store(true, AtomicOrdering::SeqCst);
                        return None;
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
                        if rcmd_press_start_time.get().is_none() {
                            rcmd_press_start_time.set(Some(Instant::now()));
                        }
                        // If RCMD is pressed while a double-tap is being awaited (first_tap_release_time is Some),
                        // it means the user didn't release RCMD cleanly between taps, or it's a new press after timeout.
                        // We should reset the double-tap expectation.
                        // This case is less about a "clean" second press and more about interrupting a pending double tap state.
                        if first_tap_release_time_for_double_tap.get().is_some() && rcmd_press_start_time.get().is_some() {
                            // If a new press starts *while* we are waiting for a second tap (first_tap_release_time_for_double_tap is Some)
                            // it's complex. For now, let's assume a new press *after* a first tap's release should not immediately clear state.
                            // The release logic will handle timeouts.
                            // The key is that rcmd_press_start_time is for the CURRENT press.
                        }
                    } else { // RCMD Released
                        let current_release_time = Instant::now();
                        if let Some(press_time) = rcmd_press_start_time.take() { 
                            if press_time.elapsed() < Duration::from_millis(RCMD_TAP_DURATION_MS as u64) {
                                let cursor_pos = match Mouse::new().get_position() {
                                    Ok(point) => Some(eframe::egui::pos2(point.x as f32, point.y as f32)),
                                    Err(_) => None,
                                };

                                if let Some(prev_release_time) = first_tap_release_time_for_double_tap.take() {
                                    if current_release_time.duration_since(prev_release_time) < Duration::from_millis(RCMD_DOUBLE_TAP_MAX_DELAY_MS as u64) {
                                        println!("RCmd Double Tap detected by listener.");
                                        let _ = shared_state.event_tx.send(GlobalEvent::RCmdDoubleTap);
                                        return None;
                                    } else {
                                        println!("Second RCmd tap too late for double. Treating as new first potential tap.");
                                        first_tap_release_time_for_double_tap.set(Some(current_release_time));
                                        let _ = shared_state.event_tx.send(GlobalEvent::PotentialSingleRCmdTap { tap_time: current_release_time, cursor_pos });
                                        return None;
                                    }
                                } else {
                                    println!("First RCmd tap release detected by listener.");
                                    first_tap_release_time_for_double_tap.set(Some(current_release_time));
                                    let _ = shared_state.event_tx.send(GlobalEvent::PotentialSingleRCmdTap { tap_time: current_release_time, cursor_pos });
                                    return None;
                                }
                            } else {
                                println!("RCmd held too long, not a tap. Cancelling pending sequence.");
                                if first_tap_release_time_for_double_tap.take().is_some() {
                                    let _ = shared_state.event_tx.send(GlobalEvent::CancelPendingRCmdTap);
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
                    if rcmd_press_start_time.get().is_some() {
                        println!("Other modifier changed while RCmd pressed, cancelling pending RCmd tap sequence.");
                        rcmd_press_start_time.set(None);
                        if first_tap_release_time_for_double_tap.take().is_some() {
                            let _ = shared_state.event_tx.send(GlobalEvent::CancelPendingRCmdTap);
                        }
                    }
                }
            }
            CGEventType::KeyDown => {
                let key_code = event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE);
                
                if rcmd_press_start_time.get().is_some() && !is_modifier_key_code(key_code) && key_code != RIGHT_COMMAND_KEY_CODE {
                    println!("Non-modifier key pressed while RCmd held, cancelling pending RCmd tap sequence.");
                    rcmd_press_start_time.set(None);
                    if first_tap_release_time_for_double_tap.take().is_some() {
                        let _ = shared_state.event_tx.send(GlobalEvent::CancelPendingRCmdTap);
                    }
                }
            }
            _ => {}
        }
        Some(event.clone())
    };

    let tap_result = CGEventTap::new(
        CGEventTapLocation::HID,        
        CGEventTapPlacement::HeadInsertEventTap, 
        CGEventTapOptions::ListenOnly,  
        vec![CGEventType::KeyDown, CGEventType::KeyUp, CGEventType::FlagsChanged], 
        callback_closure,
    );

    match tap_result {
        Ok(tap) => {
            unsafe {
                let mach_port_ref = tap.mach_port.as_concrete_TypeRef();
                let source = CFMachPortCreateRunLoopSource(ptr::null_mut(), mach_port_ref, 0);
                if source.is_null() {
                    eprintln!("Failed to create run loop source");
                    return;
                }
                let cf_run_loop_source = CFRunLoopSource::wrap_under_get_rule(source);
                
                current_run_loop.add_source(&cf_run_loop_source, kCFRunLoopCommonModes);
                
                tap.enable();
            }
            println!("Event tap enabled");
            CFRunLoop::run_current(); 
            println!("Event loop exited"); 
        }
        Err(e) => {
            eprintln!("Failed to create event tap: {:?}", e);
        }
    }
} 