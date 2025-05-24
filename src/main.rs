// Allow clippy warnings from the objc crate macros
#![allow(unexpected_cfgs)]

mod app_ui;
mod event_handler;
mod grid;

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::thread;
use std::sync::mpsc::{channel, Sender, Receiver}; 

use eframe::NativeOptions;
use objc::{msg_send, sel, sel_impl, class}; 
use objc::runtime::Object;
#[cfg(target_os = "macos")]
use cocoa::appkit::NSApplicationActivationPolicy;

use app_ui::{MouselessApp, EframeControl};
use event_handler::{global_event_listener_thread, EventTapSharedState, GlobalEvent};

fn main() -> Result<(), String> { 
    println!("Starting mouseless");

    let (event_tx, event_rx): (Sender<GlobalEvent>, Receiver<GlobalEvent>) = channel();
    let eframe_control = EframeControl::default(); 
    let lshift_key_is_pressed_arc = Arc::new(AtomicBool::new(false));

    let listener_shared_state = EventTapSharedState {
        event_tx: event_tx.clone(),
        app_is_visible: eframe_control.is_visible.clone(),
        eframe_hide_requested_by_listener: eframe_control.hide_requested.clone(),
        lshift_key_is_pressed: lshift_key_is_pressed_arc.clone(),
    };

    thread::spawn(move || {
        global_event_listener_thread(listener_shared_state);
    });
    println!("Global event listener spawned");

    let placeholder_initial_rect = eframe::egui::Rect::from_min_size(eframe::egui::Pos2::ZERO, eframe::egui::vec2(100.0,100.0));

    let native_options = NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_transparent(true)
            .with_decorations(false)
            .with_always_on_top()
            .with_maximized(true) 
            .with_visible(false) 
            .with_title("Mouseless Overlay"),
        ..Default::default()
    };
    
    println!("Starting eframe app (initially hidden)");
    let eframe_control_clone_for_app = eframe_control.clone();
    let lshift_arc_clone_for_app = lshift_key_is_pressed_arc.clone();

    let run_result = eframe::run_native(
        "Mouseless",
        native_options,
        Box::new(move |cc| {
            #[cfg(target_os = "macos")]
            unsafe {
                let ns_app_class = class!(NSApplication);
                let ns_app: *mut Object = msg_send![ns_app_class, sharedApplication];
                let _: () = msg_send![ns_app, setActivationPolicy: NSApplicationActivationPolicy::NSApplicationActivationPolicyAccessory];
                println!("Set app as accessory (won't appear in dock)");
            }
            Box::new(MouselessApp::new(cc, eframe_control_clone_for_app, placeholder_initial_rect, event_rx, lshift_arc_clone_for_app))
        }),
    );

    if let Err(e) = run_result {
        eprintln!("App error: {:?}", e);
        return Err(format!("Eframe error: {:?}", e));
    }

    println!("App exited successfully");
    Ok(())
} 