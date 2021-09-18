use rg3d::{
    core::instant::Instant,
    engine::Engine,
    event::{DeviceEvent, Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    gui::node::StubNode,
    window::WindowBuilder,
};

type GameEngine = Engine<(), StubNode>;

fn main() {
    let window_builder = WindowBuilder::new().with_title("RustCycles");
    let event_loop = EventLoop::new();
    // LATER no vsync
    let mut engine = GameEngine::new(window_builder, &event_loop, true).unwrap();

    // TODO using f32 for time might lead to instability if a match is left running for a day or so
    let clock = Instant::now();
    let mut game_time = 0.0;
    let dt = 1.0 / 60.0; // TODO configurable

    event_loop.run(move |event, _, control_flow| {
        // Default control_flow is ControllFlow::Poll but let's be explicit in acse it changes.
        *control_flow = ControlFlow::Poll;
        // This is great because we get events almost immediately,
        // e.g. 70-80 times each *milli*second when doing nothing else beside printing their times.
        // The downside is we occupy a full CPU core just for input.
        // TODO Send important input to server immediately (keyboard and mouse button changes, mouse movement up to those changes)
        // LATER Is there a way to not waste CPU cycles so much?

        match event {
            Event::NewEvents(_) => {}
            Event::WindowEvent { event, .. } => {
                match event {
                    WindowEvent::Resized(size) => {
                        engine.renderer.set_frame_size(size.into());
                    }
                    WindowEvent::CloseRequested => {
                        *control_flow = ControlFlow::Exit;
                    }
                    WindowEvent::ReceivedCharacter(_) => {
                        // LATER might be useful for console/chat?
                    }
                    WindowEvent::Focused(focus) => {
                        println!("{} focus {:?}", clock.elapsed().as_secs_f32(), focus);
                        // LATER pause/unpause
                    }
                    WindowEvent::KeyboardInput { input, .. } => {
                        println!(
                            "{} keyboard input {:?}",
                            clock.elapsed().as_secs_f32(),
                            input
                        );
                    }
                    WindowEvent::MouseWheel { delta, phase, .. } => {
                        println!(
                            "{} wheel {:?} {:?}",
                            clock.elapsed().as_secs_f32(),
                            delta,
                            phase
                        );
                    }
                    WindowEvent::MouseInput { state, button, .. } => {
                        println!(
                            "{} mouse input {:?} {:?}",
                            clock.elapsed().as_secs_f32(),
                            state,
                            button
                        );
                    }
                    _ => {}
                }
            }
            // Using device event for mouse motion because
            // - it reports delta, not position
            // - it doesn't care whether we're at the edge of the screen
            // TODO(privacy) make sure we're not handling mouse movement when minimized
            Event::DeviceEvent { event, .. } => {
                match event {
                    DeviceEvent::MouseMotion { delta } => {
                        // LATER This event normally happens every 4 ms for me when moving the mouse. Print stats.
                        // Is it limited by my polling rate? Would it be helpful to teach players how to increase it?
                        // Sometimes i get 4 events every 16 ms. Detect this.
                        // https://github.com/martin-t/rustcycles/issues/1
                        println!(
                            "{} DeviceEvent::MouseMotion {:?}",
                            clock.elapsed().as_secs_f32(),
                            delta
                        );
                    }
                    _ => {}
                }
            }
            Event::UserEvent(_) => {}
            // LATER test suspend/resume
            Event::Suspended => {}
            Event::Resumed => {}
            Event::MainEventsCleared => {
                // TODO read these (again), verify what works best in practise:
                // https://gafferongames.com/post/fix_your_timestep/
                // https://medium.com/@tglaiel/how-to-make-your-game-run-at-60fps-24c61210fe75

                let game_time_target = clock.elapsed().as_secs_f32();
                while game_time + dt < game_time_target {
                    game_time += dt;

                    // TODO gamelogic here

                    engine.update(dt);
                }

                engine.get_window().request_redraw();
            }
            Event::RedrawRequested(_) => {
                engine.render().unwrap(); // LATER only crash if failed multiple times
            }
            Event::RedrawEventsCleared => {}
            Event::LoopDestroyed => println!("bye"),
        }
    });
}
