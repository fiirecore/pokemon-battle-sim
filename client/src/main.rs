extern crate firecore_battle_net as common;

use std::{net::SocketAddr, rc::Rc};

use common::game::{
    battle::gui::BattlePlayerGui,
    graphics::draw_text_left,
    gui::{bag::BagGui, party::PartyGui},
    log::{info, warn},
    tetra::{
        graphics::{
            self,
            Color,
            scaling::{ScalingMode, ScreenScaler},
        },
        input::{self, Key},
        time::{self, Timestep},
        Context, Event, Result, State,
        ContextBuilder,
    },
    util::{WIDTH, HEIGHT},
    init,
    deps::ser,
};

use self::sender::BattleConnection;

mod sender;

const SCALE: f32 = 3.0;
const TITLE: &str = "Pokemon Battle";

fn main() -> Result {
    common::init();
    ContextBuilder::new(
        TITLE,
        (WIDTH * SCALE) as _,
        (HEIGHT * SCALE) as _,
    )
    .vsync(true)
    .resizable(true)
    .show_mouse(true)
    .timestep(Timestep::Variable)
    .build()
    .unwrap()
    .run(GameState::new)
}

enum States {
    Connect(String),
    Connected(BattleConnection, ConnectState),
}

enum ConnectState {
    WaitConfirm,
    // WaitBegin,
    Connected,
}

struct GameState {
    state: States,
    gui: BattlePlayerGui,
    scaler: ScreenScaler,
}

impl GameState {
    pub fn new(ctx: &mut Context) -> Result<Self> {
        let party = Rc::new(PartyGui::new(ctx));
        let bag = Rc::new(BagGui::new(ctx));

        let scaler = ScreenScaler::with_window_size(
            ctx,
            WIDTH as _,
            HEIGHT as _,
            ScalingMode::ShowAll,
        )?;
        Ok(Self {
            state: States::Connect(String::new()),
            gui: BattlePlayerGui::new(ctx, party, bag),
            scaler,
        })
    }
}

impl State for GameState {
    fn begin(&mut self, ctx: &mut Context) -> Result {
        init::configuration()?;
        init::text(
            ctx,
            ser::deserialize(include_bytes!("../../../pokemon-game/build/data/fonts.bin"))
                .unwrap(),
        )?;
        init::pokedex(ctx, ser::deserialize(common::DEX_BYTES).unwrap())
    }

    fn end(&mut self, ctx: &mut Context) -> Result {
        Ok(())
    }

    fn update(&mut self, ctx: &mut Context) -> Result {
        match &mut self.state {
            States::Connect(string) => {
                if input::is_key_pressed(ctx, Key::Backspace) {
                    string.pop();
                }
                if input::is_key_pressed(ctx, Key::Enter) {
                    let mut strings = string.split_ascii_whitespace();
                    if let Some(ip) = strings.next() {
                        match ip.parse::<SocketAddr>() {
                            Ok(addr) => {
                                info!("Connecting to server at {}", addr);
                                self.state = States::Connected(
                                    BattleConnection::connect(addr, strings.next().map(ToOwned::to_owned)),
                                    ConnectState::WaitConfirm,
                                );
                            }
                            Err(err) => {
                                warn!("Could not parse ip address with error {}", err);
                                string.clear();
                            }
                        }
                    } else {
                        warn!("No text was input for IP.");
                    }
                } else if let Some(new) = input::get_text_input(ctx) {
                    string.push_str(new);
                }
            }
            States::Connected(connection, connected) => {
                connection.poll();
                match connected {
                    ConnectState::WaitConfirm => {
                        if connection.wait_confirm() {
                            *connected = ConnectState::Connected;
                        }
                    }
                    // ConnectState::WaitBegin => {
                    //     if connection.wait_begin() {
                    //         *connected = ConnectState::Connected;
                    //     }
                    // }
                    ConnectState::Connected => {
                        connection.receive(&mut self.gui, ctx);
                        self.gui
                            .update(ctx, time::get_delta_time(ctx).as_secs_f32());
                        connection.send(&mut self.gui);
                    }
                }
            }
        }
        Ok(())
    }

    fn draw(&mut self, ctx: &mut Context) -> Result {
        graphics::clear(ctx, Color::BLACK);
        {
            match &self.state {
                States::Connect(ip) => draw_text_left(ctx, &1, ip, &Color::WHITE, 5.0, 5.0),
                States::Connected(.., connected) => match connected {
                    ConnectState::WaitConfirm => {
                        draw_text_left(ctx, &1, "Connecting...", &Color::WHITE, 5.0, 5.0)
                    }
                    _ => {
                        graphics::set_canvas(ctx, self.scaler.canvas());
                        graphics::clear(ctx, Color::BLACK);
                        self.gui.draw(ctx);
                        graphics::reset_transform_matrix(ctx);
                        graphics::reset_canvas(ctx);
                        self.scaler.draw(ctx);
                    }
                },
            }
        }
        Ok(())
    }

    fn event(&mut self, ctx: &mut Context, event: Event) -> Result {
        Ok(())
    }
}
