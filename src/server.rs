use crate::{
    common::{GameState, Input},
    GameEngine,
};

pub(crate) struct Server {
    pub(crate) engine: GameEngine,
    pub(crate) gs: GameState,
    pub(crate) input: Input,
}

impl Server {
    pub(crate) async fn new(mut engine: GameEngine) -> Self {
        let gs = GameState::new(&mut engine).await;

        Self {
            engine,
            gs,
            input: Input::default(),
        }
    }

    pub(crate) fn update(&mut self, game_time_target: f32) {
        // This is similar to Client::update,
        // see that for more information.

        let dt = 1.0 / 60.0;
        let game_time_target = game_time_target;

        while self.gs.game_time + dt < game_time_target {
            self.gs.game_time += dt;

            // TODO input
            self.gs.tick(&mut self.engine, dt, self.input);

            self.engine.update(dt);
        }

        // TODO Send updates to clients
    }
}
