#![cfg(target_os = "linux")]

use std::thread;
use std::time::Duration;

use evdev::{AbsoluteAxisCode, EventType, InputEvent, SynchronizationCode};
use touchpadflow::action::{BrightnessController, VolumeController};
use touchpadflow::config::{AppConfig, EdgeAction};
use touchpadflow::device::{TouchpadMetrics, VirtualOutput};
use touchpadflow::gesture::{GestureEngine, SessionMode};

fn make_test_engine() -> (GestureEngine, VolumeController, BrightnessController) {
    let config = AppConfig::default();
    let metrics = TouchpadMetrics {
        min_x: 0,
        max_x: 1000,
        min_y: 0,
        max_y: 1000,
    };
    let volume = VolumeController::detached();
    let brightness = BrightnessController::detached();
    let output = VirtualOutput::dummy();

    let engine = GestureEngine::new(config, metrics, volume.clone(), brightness.clone(), output);
    (engine, volume, brightness)
}

fn ev_slot(slot: i32) -> InputEvent {
    InputEvent::new(EventType::ABSOLUTE.0, AbsoluteAxisCode::ABS_MT_SLOT.0, slot)
}

fn ev_tracking_id(id: i32) -> InputEvent {
    InputEvent::new(
        EventType::ABSOLUTE.0,
        AbsoluteAxisCode::ABS_MT_TRACKING_ID.0,
        id,
    )
}

fn ev_x(x: i32) -> InputEvent {
    InputEvent::new(
        EventType::ABSOLUTE.0,
        AbsoluteAxisCode::ABS_MT_POSITION_X.0,
        x,
    )
}

fn ev_y(y: i32) -> InputEvent {
    InputEvent::new(
        EventType::ABSOLUTE.0,
        AbsoluteAxisCode::ABS_MT_POSITION_Y.0,
        y,
    )
}

fn ev_syn() -> InputEvent {
    InputEvent::new(
        EventType::SYNCHRONIZATION.0,
        SynchronizationCode::SYN_REPORT.0,
        0,
    )
}

#[test]
fn test_single_finger_pointer_mode() {
    let (mut engine, _, _) = make_test_engine();

    // Finger 0 touches center (x=500, y=500)
    engine.handle_event(ev_slot(0)).unwrap();
    engine.handle_event(ev_tracking_id(10)).unwrap();
    engine.handle_event(ev_x(500)).unwrap();
    engine.handle_event(ev_y(500)).unwrap();
    engine.handle_event(ev_syn()).unwrap();

    // Move finger
    engine.handle_event(ev_slot(0)).unwrap();
    engine.handle_event(ev_x(520)).unwrap();
    engine.handle_event(ev_y(530)).unwrap();
    engine.handle_event(ev_syn()).unwrap();

    // Must be settling or pointer
    assert_ne!(engine.session_mode(), SessionMode::EdgeVolume);
    assert_ne!(engine.session_mode(), SessionMode::EdgeBrightness);

    // Release
    engine.handle_event(ev_slot(0)).unwrap();
    engine.handle_event(ev_tracking_id(-1)).unwrap();
    engine.handle_event(ev_syn()).unwrap();

    assert_eq!(engine.session_mode(), SessionMode::Pointer);
}

#[test]
fn test_two_finger_right_edge_volume_control() {
    let (mut engine, volume, _) = make_test_engine();
    let initial_vol = volume.target_value();

    // Finger 0 and Finger 1 contact right edge (x >= 800)
    engine.handle_event(ev_slot(0)).unwrap();
    engine.handle_event(ev_tracking_id(1)).unwrap();
    engine.handle_event(ev_x(850)).unwrap();
    engine.handle_event(ev_y(600)).unwrap();

    engine.handle_event(ev_slot(1)).unwrap();
    engine.handle_event(ev_tracking_id(2)).unwrap();
    engine.handle_event(ev_x(900)).unwrap();
    engine.handle_event(ev_y(600)).unwrap();
    engine.handle_event(ev_syn()).unwrap();

    assert_eq!(engine.session_mode(), SessionMode::EdgeVolume);

    // Move upward (y decreases: volume up)
    for y in (400..=550).rev().step_by(30) {
        engine.handle_event(ev_slot(0)).unwrap();
        engine.handle_event(ev_y(y)).unwrap();
        engine.handle_event(ev_slot(1)).unwrap();
        engine.handle_event(ev_y(y)).unwrap();
        engine.handle_event(ev_syn()).unwrap();
    }

    let final_vol = volume.target_value();
    assert!(
        final_vol > initial_vol,
        "Volume should increase after upward slide on right edge: initial={}, final={}",
        initial_vol,
        final_vol
    );

    // Release fingers
    engine.handle_event(ev_slot(0)).unwrap();
    engine.handle_event(ev_tracking_id(-1)).unwrap();
    engine.handle_event(ev_slot(1)).unwrap();
    engine.handle_event(ev_tracking_id(-1)).unwrap();
    engine.handle_event(ev_syn()).unwrap();

    assert_eq!(engine.session_mode(), SessionMode::Pointer);
}

#[test]
fn test_two_finger_left_edge_brightness_control_with_natural_centroid() {
    let (mut engine, _, brightness) = make_test_engine();
    let initial_bright = brightness.target_value();

    // Finger 0 at x=100 (10%), Finger 1 at x=240 (24%)
    // Average x = 170 (17% <= 22% left edge). Natural human finger spread!
    engine.handle_event(ev_slot(0)).unwrap();
    engine.handle_event(ev_tracking_id(1)).unwrap();
    engine.handle_event(ev_x(100)).unwrap();
    engine.handle_event(ev_y(500)).unwrap();

    engine.handle_event(ev_slot(1)).unwrap();
    engine.handle_event(ev_tracking_id(2)).unwrap();
    engine.handle_event(ev_x(240)).unwrap();
    engine.handle_event(ev_y(500)).unwrap();
    engine.handle_event(ev_syn()).unwrap();

    assert_eq!(
        engine.session_mode(),
        SessionMode::EdgeBrightness,
        "Centroid edge detection should engage EdgeBrightness even with wide finger spacing"
    );

    // Slide upward to increase brightness
    for y in (350..=480).rev().step_by(25) {
        engine.handle_event(ev_slot(0)).unwrap();
        engine.handle_event(ev_y(y)).unwrap();
        engine.handle_event(ev_slot(1)).unwrap();
        engine.handle_event(ev_y(y)).unwrap();
        engine.handle_event(ev_syn()).unwrap();
    }

    let final_bright = brightness.target_value();
    assert!(
        final_bright > initial_bright,
        "Brightness should increase after upward slide: initial={}, final={}",
        initial_bright,
        final_bright
    );
}

#[test]
fn test_edge_trigger_threshold_deadband() {
    let (mut engine, volume, _) = make_test_engine();
    let initial_vol = volume.target_value();

    // Touch down on right edge
    engine.handle_event(ev_slot(0)).unwrap();
    engine.handle_event(ev_tracking_id(1)).unwrap();
    engine.handle_event(ev_x(880)).unwrap();
    engine.handle_event(ev_y(500)).unwrap();

    engine.handle_event(ev_slot(1)).unwrap();
    engine.handle_event(ev_tracking_id(2)).unwrap();
    engine.handle_event(ev_x(920)).unwrap();
    engine.handle_event(ev_y(500)).unwrap();
    engine.handle_event(ev_syn()).unwrap();

    // Jitter only 10px vertically (less than default edge_trigger_threshold = 35px)
    engine.handle_event(ev_slot(0)).unwrap();
    engine.handle_event(ev_y(490)).unwrap();
    engine.handle_event(ev_slot(1)).unwrap();
    engine.handle_event(ev_y(490)).unwrap();
    engine.handle_event(ev_syn()).unwrap();

    assert_eq!(
        volume.target_value(),
        initial_vol,
        "Sub-threshold motion (< 35px) should not trigger volume change"
    );

    // Move past threshold (80px change)
    engine.handle_event(ev_slot(0)).unwrap();
    engine.handle_event(ev_y(420)).unwrap();
    engine.handle_event(ev_slot(1)).unwrap();
    engine.handle_event(ev_y(420)).unwrap();
    engine.handle_event(ev_syn()).unwrap();

    assert!(
        volume.target_value() > initial_vol,
        "Motion exceeding edge_trigger_threshold should apply step adjustment"
    );
}

#[test]
fn test_resting_finger_is_not_reset_after_idle_gap() {
    // Regression: on battery, pads stop reporting a still finger. The old 400ms watchdog
    // then "lifted" it mid-touch, which made the cursor freeze and jump.
    let (mut engine, _, _) = make_test_engine();

    engine.handle_event(ev_slot(0)).unwrap();
    engine.handle_event(ev_tracking_id(1)).unwrap();
    engine.handle_event(ev_x(500)).unwrap();
    engine.handle_event(ev_y(500)).unwrap();
    engine.handle_event(ev_syn()).unwrap();
    assert_eq!(engine.session_mode(), SessionMode::Pointer);

    thread::sleep(Duration::from_millis(450));

    // Second finger lands at the right edge after the gap: still the same pointer session.
    engine.handle_event(ev_slot(1)).unwrap();
    engine.handle_event(ev_tracking_id(2)).unwrap();
    engine.handle_event(ev_x(950)).unwrap();
    engine.handle_event(ev_y(500)).unwrap();
    engine.handle_event(ev_syn()).unwrap();
    assert_eq!(engine.session_mode(), SessionMode::Pointer);
}

#[test]
fn test_edge_actions_follow_config_and_none_disables_zone() {
    let config = AppConfig {
        left_action: EdgeAction::Volume,
        right_action: EdgeAction::None,
        ..AppConfig::default()
    };
    let metrics = TouchpadMetrics {
        min_x: 0,
        max_x: 1000,
        min_y: 0,
        max_y: 1000,
    };
    let volume = VolumeController::detached();
    let brightness = BrightnessController::detached();
    let mut engine =
        GestureEngine::new(config, metrics, volume, brightness, VirtualOutput::dummy());

    let two_fingers = |engine: &mut GestureEngine, x: i32, id: i32| {
        engine.handle_event(ev_slot(0)).unwrap();
        engine.handle_event(ev_tracking_id(id)).unwrap();
        engine.handle_event(ev_x(x)).unwrap();
        engine.handle_event(ev_y(500)).unwrap();
        engine.handle_event(ev_slot(1)).unwrap();
        engine.handle_event(ev_tracking_id(id + 1)).unwrap();
        engine.handle_event(ev_x(x + 30)).unwrap();
        engine.handle_event(ev_y(500)).unwrap();
        engine.handle_event(ev_syn()).unwrap();
    };
    let lift = |engine: &mut GestureEngine| {
        engine.handle_event(ev_slot(0)).unwrap();
        engine.handle_event(ev_tracking_id(-1)).unwrap();
        engine.handle_event(ev_slot(1)).unwrap();
        engine.handle_event(ev_tracking_id(-1)).unwrap();
        engine.handle_event(ev_syn()).unwrap();
    };

    two_fingers(&mut engine, 50, 1);
    assert_eq!(engine.session_mode(), SessionMode::EdgeVolume);
    lift(&mut engine);

    two_fingers(&mut engine, 920, 10);
    assert_eq!(engine.session_mode(), SessionMode::Pointer);
}

/// Three fingers down at (x, 500), then slide them horizontally through `xs`, then lift.
fn three_finger_slide(engine: &mut GestureEngine, start: i32, xs: &[i32]) {
    for (slot, id) in [(0, 21), (1, 22), (2, 23)] {
        engine.handle_event(ev_slot(slot)).unwrap();
        engine.handle_event(ev_tracking_id(id)).unwrap();
        engine.handle_event(ev_x(start + slot * 40)).unwrap();
        engine.handle_event(ev_y(500)).unwrap();
    }
    engine.handle_event(ev_syn()).unwrap();
    for &x in xs {
        for slot in 0..3 {
            engine.handle_event(ev_slot(slot)).unwrap();
            engine.handle_event(ev_x(x + slot * 40)).unwrap();
        }
        engine.handle_event(ev_syn()).unwrap();
    }
    for slot in 0..3 {
        engine.handle_event(ev_slot(slot)).unwrap();
        engine.handle_event(ev_tracking_id(-1)).unwrap();
    }
    engine.handle_event(ev_syn()).unwrap();
}

fn macro_rule(gesture: &str, action: &str) -> touchpadflow::config::MacroRule {
    touchpadflow::config::MacroRule {
        enabled: true,
        name: String::new(),
        apps: Vec::new(),
        gesture: gesture.into(),
        action: action.into(),
    }
}

#[test]
fn test_macro_swipe_repeats_with_distance_and_reverses() {
    let config = AppConfig {
        macros: vec![
            macro_rule("swipe3_right", "ctrl+tab"),
            macro_rule("swipe3_left", "ctrl+shift+tab"),
        ],
        ..AppConfig::default()
    };
    let metrics = TouchpadMetrics {
        min_x: 0,
        max_x: 1000,
        min_y: 0,
        max_y: 1000,
    };
    let mut engine = GestureEngine::new(
        config,
        metrics,
        VolumeController::detached(),
        BrightnessController::detached(),
        VirtualOutput::dummy(),
    );

    // Default discrete_threshold is 120: slide right 3 steps (360+), then back 2 steps.
    let path: Vec<i32> = (300..=680)
        .step_by(20)
        .chain((440..=660).rev().step_by(20))
        .collect();
    three_finger_slide(&mut engine, 300, &path);

    let taps = &engine.output().tapped;
    let shift = evdev::KeyCode::KEY_LEFTSHIFT;
    let forward = taps
        .iter()
        .filter(|(m, k)| *k == evdev::KeyCode::KEY_TAB && !m.contains(&shift))
        .count();
    let back = taps
        .iter()
        .filter(|(m, k)| *k == evdev::KeyCode::KEY_TAB && m.contains(&shift))
        .count();
    assert_eq!(forward, 3, "three tabs forward, got {taps:?}");
    assert_eq!(back, 2, "two tabs back, got {taps:?}");
}

#[test]
fn test_built_in_shortcut_swipe_still_fires_once() {
    let (mut engine, _, _) = make_test_engine();
    // Default swipe3_right is key:KEY_NEXTSONG; a long slide must not skip several tracks.
    let path: Vec<i32> = (300..=760).step_by(20).collect();
    three_finger_slide(&mut engine, 300, &path);
    assert_eq!(
        engine.output().tapped.len(),
        1,
        "{:?}",
        engine.output().tapped
    );
}
