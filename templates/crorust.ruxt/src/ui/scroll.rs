use egui::{Context, Event, Id, MouseWheelUnit, NumExt as _, RawInput, Rect, Response, Vec2};

/// Multiplier applied to smooth touchpad/trackpad point deltas.
/// Calibrated to provide 1:1 parity with standard browser viewport velocity.
pub const TRACKPAD_SCROLL_MULTIPLIER: f32 = 2.25;

/// Desktop-standard line scroll speed for discrete mouse wheels.
pub const DESKTOP_LINE_SCROLL_SPEED: f32 = 40.0;

/// Retained state for touch-fling momentum physics.
#[derive(Clone, Copy, Debug, Default)]
pub struct MomentumState {
    pub velocity: f32,
    pub is_dragging: bool,
}

/// Normalizes raw input events before they enter the egui reactive frame pulse.
/// Multiplies `MouseWheelUnit::Point` (from precision laptop trackpads) to eliminate
/// sluggish trackpad scrolling while keeping discrete mouse wheels (`MouseWheelUnit::Line`) untouched.
pub fn tune_raw_input(raw_input: &mut RawInput) {
    for event in &mut raw_input.events {
        if let Event::MouseWheel { unit, delta, .. } = event
            && *unit == MouseWheelUnit::Point
        {
            *delta *= TRACKPAD_SCROLL_MULTIPLIER;
        }
    }
}

/// Applies global context options to eliminate WebAssembly slow scroll defaults.
pub fn configure_input_options(ctx: &Context) {
    ctx.options_mut(|opt| {
        if opt.input_options.line_scroll_speed < DESKTOP_LINE_SCROLL_SPEED {
            opt.input_options.line_scroll_speed = DESKTOP_LINE_SCROLL_SPEED;
        }
    });
}

/// Applies ergonomic momentum physics to a vertical ScrollArea on mobile/touch interfaces.
pub fn assist_touch_scroll(
    ctx: &Context,
    scroll_id: Id,
    content_response: Option<&Response>,
    inner_rect: Rect,
    content_size: Vec2,
) {
    let dt = ctx.input(|i| i.stable_dt).at_most(0.1);
    let physics_id = Id::new("touch_physics").with(scroll_id);
    let mut momentum: MomentumState = ctx.data_mut(|d| *d.get_temp_mut_or_default(physics_id));

    let max_offset_y = (content_size.y - inner_rect.height()).max(0.0);

    // Read pointer state to detect touch dragging and release velocity
    let is_pointer_down =
        ctx.input(|i| i.pointer.is_decidedly_dragging() || i.pointer.primary_down());
    let current_vel = ctx.input(|i| i.pointer.velocity().y);

    if let Some(response) = content_response {
        if response.dragged() {
            momentum.is_dragging = true;
            momentum.velocity = current_vel;
        } else if response.drag_stopped() {
            momentum.is_dragging = false;
            let release_vel = current_vel;
            if release_vel.abs() > 30.0 {
                momentum.velocity = release_vel * 1.35;
            }
        }
    } else if is_pointer_down {
        // If no explicit response was passed, track pointer state directly
        momentum.is_dragging = true;
        momentum.velocity = current_vel;
    } else if momentum.is_dragging {
        // Pointer just lifted
        momentum.is_dragging = false;
        if momentum.velocity.abs() > 30.0 {
            momentum.velocity *= 1.35;
        }
    }

    // Apply coasting if not currently dragging and velocity is significant
    if !momentum.is_dragging
        && momentum.velocity.abs() > 2.0
        && let Some(mut state) = egui::scroll_area::State::load(ctx, scroll_id)
    {
        let delta_offset = -momentum.velocity * dt;
        let target_offset = (state.offset.y + delta_offset).clamp(0.0, max_offset_y);

        if target_offset <= 0.0 || target_offset >= max_offset_y {
            momentum.velocity = 0.0;
        } else {
            let decay = (-dt / 0.35).exp();
            momentum.velocity *= decay;
            ctx.request_repaint();
        }

        state.offset.y = target_offset;
        state.store(ctx, scroll_id);
    }

    ctx.data_mut(|d| *d.get_temp_mut_or_default(physics_id) = momentum);
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Event, Modifiers, MouseWheelUnit, RawInput, TouchPhase, vec2};

    #[test]
    fn test_trackpad_point_delta_scaled() {
        let mut raw_input = RawInput::default();
        raw_input.events.push(Event::MouseWheel {
            unit: MouseWheelUnit::Point,
            delta: vec2(0.0, 5.0),
            modifiers: Modifiers::NONE,
            phase: TouchPhase::Move,
        });

        tune_raw_input(&mut raw_input);

        if let Event::MouseWheel { unit, delta, .. } = &raw_input.events[0] {
            assert_eq!(*unit, MouseWheelUnit::Point);
            assert_eq!(*delta, vec2(0.0, 5.0 * TRACKPAD_SCROLL_MULTIPLIER));
        } else {
            panic!("Expected MouseWheel event");
        }
    }

    #[test]
    fn test_mouse_wheel_line_delta_unmodified() {
        let mut raw_input = RawInput::default();
        raw_input.events.push(Event::MouseWheel {
            unit: MouseWheelUnit::Line,
            delta: vec2(0.0, 1.0),
            modifiers: Modifiers::NONE,
            phase: TouchPhase::Move,
        });

        tune_raw_input(&mut raw_input);

        if let Event::MouseWheel { unit, delta, .. } = &raw_input.events[0] {
            assert_eq!(*unit, MouseWheelUnit::Line);
            assert_eq!(*delta, vec2(0.0, 1.0));
        } else {
            panic!("Expected MouseWheel event");
        }
    }

    #[test]
    fn test_momentum_decay_reaches_rest() {
        let mut vel: f32 = 500.0;
        let dt: f32 = 0.016;
        let mut total_time: f32 = 0.0;

        while vel.abs() > 2.0 {
            let decay = (-dt / 0.35).exp();
            vel *= decay;
            total_time += dt;
            assert!(
                total_time < 3.0,
                "Momentum must come to rest within 3 seconds"
            );
        }

        assert!(total_time >= 0.5, "Momentum should glide for at least 0.5s");
    }

    #[test]
    fn test_assist_touch_scroll_coasting_and_repaint_request() {
        let ctx = Context::default();
        let mut output = ctx.run_ui(RawInput::default(), |_| {});
        output.textures_delta.clear();

        let scroll_id = Id::new("test_scroll_area");
        let physics_id = Id::new("touch_physics").with(scroll_id);

        let mut initial_state = egui::scroll_area::State::default();
        initial_state.offset.y = 500.0;
        initial_state.store(&ctx, scroll_id);

        let momentum = MomentumState {
            velocity: 400.0,
            is_dragging: false,
        };
        ctx.data_mut(|d| *d.get_temp_mut_or_default(physics_id) = momentum);

        let inner_rect = Rect::from_min_size(egui::Pos2::ZERO, vec2(400.0, 600.0));
        let content_size = vec2(400.0, 2000.0);

        assist_touch_scroll(&ctx, scroll_id, None, inner_rect, content_size);

        let updated_state = egui::scroll_area::State::load(&ctx, scroll_id).unwrap();
        assert!(
            updated_state.offset.y < 500.0,
            "Negative velocity delta must advance offset upward, got {}",
            updated_state.offset.y
        );

        let updated_momentum: MomentumState =
            ctx.data_mut(|d| *d.get_temp_mut_or_default(physics_id));
        assert!(updated_momentum.velocity < 400.0 && updated_momentum.velocity > 0.0);
        assert!(ctx.has_requested_repaint());
    }

    #[test]
    fn test_assist_touch_scroll_boundary_clamping_stops_velocity() {
        let ctx = Context::default();
        let mut output = ctx.run_ui(RawInput::default(), |_| {});
        output.textures_delta.clear();

        let scroll_id = Id::new("test_boundary_scroll_area");
        let physics_id = Id::new("touch_physics").with(scroll_id);

        let inner_rect = Rect::from_min_size(egui::Pos2::ZERO, vec2(400.0, 600.0));
        let content_size = vec2(400.0, 1000.0); // max_offset_y = 400.0

        let mut initial_state = egui::scroll_area::State::default();
        initial_state.offset.y = 399.0;
        initial_state.store(&ctx, scroll_id);

        // Downward fling overshooting max_offset_y
        let momentum = MomentumState {
            velocity: -500.0,
            is_dragging: false,
        };
        ctx.data_mut(|d| *d.get_temp_mut_or_default(physics_id) = momentum);

        assist_touch_scroll(&ctx, scroll_id, None, inner_rect, content_size);

        let updated_state = egui::scroll_area::State::load(&ctx, scroll_id).unwrap();
        assert_eq!(
            updated_state.offset.y, 400.0,
            "Offset must clamp exactly to max_offset_y"
        );

        let updated_momentum: MomentumState =
            ctx.data_mut(|d| *d.get_temp_mut_or_default(physics_id));
        assert_eq!(
            updated_momentum.velocity, 0.0,
            "Velocity must be zeroed when boundary is reached"
        );
    }
}
