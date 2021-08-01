#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

extern crate firecore_battle_gui as gui;
extern crate firecore_battle_net as common;
pub use gui::pokedex;
pub use gui::pokedex::engine;

use std::{
    net::{IpAddr, SocketAddr},
    ops::{Deref, DerefMut},
    rc::Rc,
};

use common::{rand::prelude::ThreadRng, uuid::Uuid};

use engine::{
    graphics::draw_text_left,
    tetra::{
        graphics::{
            self,
            scaling::{ScalingMode, ScreenScaler},
            Color,
        },
        input::{self, Key},
        time, Context, ContextBuilder, Result, State,
    },
    util::{HEIGHT, WIDTH},
};

use pokedex::{
    context::PokedexClientContext,
    engine::{text::TextColor, EngineContext},
    gui::{bag::BagGui, party::PartyGui},
    item::bag::Bag,
};

use gui::BattlePlayerGui;

use log::{info, warn, LevelFilter};

use self::sender::BattleConnection;

mod sender;

const SCALE: f32 = 3.0;
const TITLE: &str = "Pokemon Battle";

fn main() -> Result {
    let l = simple_logger::SimpleLogger::new();

    #[cfg(debug_assertions)]
    let l = l.with_level(LevelFilter::Debug);

    #[cfg(not(debug_assertions))]
    let l = l.with_level(LevelFilter::Info);

    l.init()
        .unwrap_or_else(|err| panic!("Could not initialize logger with error {}", err));

    let mut engine = engine::build(
        ContextBuilder::new(TITLE, (WIDTH * SCALE) as _, (HEIGHT * SCALE) as _)
            .vsync(true)
            .resizable(true)
            .show_mouse(true),
        common::ser::deserialize(include_bytes!("../fonts.bin"))
            .unwrap_or_else(|err| panic!("Could not read fonts with error {}", err)),
    )?;

    let pokedex = common::ser::deserialize(include_bytes!("../dex.bin"))
        .unwrap_or_else(|err| panic!("Could not read pokedex with error {}", err));

    let pokedex = PokedexClientContext::new(&mut engine, pokedex)?;

    let mut ctx = GameContext {
        engine,
        pokedex,
        random: common::rand::thread_rng(),
        bag: Default::default(),
    };

    engine::run(&mut ctx, GameState::new)
}

pub struct GameContext {
    pub engine: EngineContext,
    pub pokedex: PokedexClientContext,
    pub random: ThreadRng,
    pub bag: Bag,
}

impl Deref for GameContext {
    type Target = Context;

    fn deref(&self) -> &Self::Target {
        &self.engine.tetra
    }
}

impl DerefMut for GameContext {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.engine.tetra
    }
}

pub enum States {
    Connect(String),
    Connected(BattleConnection, ConnectState),
}

impl States {
    pub const CONNECT: Self = Self::Connect(String::new());
}

pub enum ConnectState {
    WaitConfirm,
    // WaitBegin,
    Closed,
    WrongVersion(f32),
    ConnectedWait,
    ConnectedPlay,
}

struct GameState {
    state: States,
    gui: BattlePlayerGui<Uuid>,
    scaler: ScreenScaler,
}

impl GameState {
    pub fn new(ctx: &mut GameContext) -> Result<Self> {
        let party = Rc::new(PartyGui::new(&ctx.pokedex));
        let bag = Rc::new(BagGui::new(&ctx.pokedex));

        let mut gui = BattlePlayerGui::new(ctx, party, bag);

        gui.opponent.trainer = Some("rival".parse().unwrap());

        let scaler =
            ScreenScaler::with_window_size(ctx, WIDTH as _, HEIGHT as _, ScalingMode::ShowAll)?;
        Ok(Self {
            state: States::Connect(String::new()),
            gui,
            scaler,
        })
    }
}

impl State<GameContext> for GameState {
    fn end(&mut self, _ctx: &mut GameContext) -> Result {
        match &mut self.state {
            States::Connect(..) => (),
            States::Connected(connection, ..) => connection.end(),
        }
        Ok(())
    }

    fn update(&mut self, ctx: &mut GameContext) -> Result {
        match &mut self.state {
            States::Connect(string) => {
                if input::is_key_pressed(ctx, Key::Backspace) {
                    string.pop();
                }
                if input::is_key_pressed(ctx, Key::Enter) {
                    let mut strings = string.split_ascii_whitespace();
                    if let Some(ip) = strings.next() {
                        let addr =
                            ip.parse::<SocketAddr>()
                                .or_else(|err| match ip.parse::<IpAddr>() {
                                    Ok(addr) => Ok(SocketAddr::new(addr, common::DEFAULT_PORT)),
                                    Err(..) => Err(err),
                                });

                        match addr {
                            Ok(addr) => {
                                info!("Connecting to server at {}", addr);
                                self.state = States::Connected(
                                    BattleConnection::connect(
                                        addr,
                                        strings.next().map(ToOwned::to_owned),
                                    ),
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
            States::Connected(connection, state) => match state {
                ConnectState::WaitConfirm => {
                    if let Some(connected) = connection.wait_confirm(&mut ctx.random) {
                        *state = connected;
                    }
                }
                ConnectState::Closed => self.state = States::Connect(String::new()),
                ConnectState::ConnectedWait => connection.gui_receive(&mut self.gui, ctx, state),
                ConnectState::WrongVersion(remaining) => {
                    *remaining -= time::get_delta_time(ctx).as_secs_f32();
                    if remaining < &mut 0.0 {
                        self.state = States::Connect(String::new());
                    }
                }
                ConnectState::ConnectedPlay => {
                    connection.gui_receive(&mut self.gui, ctx, state);
                    self.gui.update(
                        &ctx.engine,
                        &ctx.pokedex,
                        time::get_delta_time(ctx).as_secs_f32(),
                        &mut ctx.bag,
                    );
                    connection.gui_send(&mut self.gui);
                }
            },
        }
        Ok(())
    }

    fn draw(&mut self, ctx: &mut GameContext) -> Result {
        graphics::clear(ctx, Color::BLACK);
        {
            match &self.state {
                States::Connect(ip) => {
                    draw_text_left(
                        &mut ctx.engine,
                        &1,
                        "Input IP Address",
                        TextColor::White,
                        5.0,
                        5.0,
                    );
                    draw_text_left(&mut ctx.engine, &1, ip, TextColor::White, 5.0, 25.0);
                }
                States::Connected(.., connected) => match connected {
                    ConnectState::WaitConfirm => draw_text_left(
                        &mut ctx.engine,
                        &1,
                        "Connecting...",
                        TextColor::White,
                        5.0,
                        5.0,
                    ),
                    ConnectState::ConnectedWait => {
                        draw_text_left(
                            &mut ctx.engine,
                            &1,
                            "Connected!",
                            TextColor::White,
                            5.0,
                            5.0,
                        );
                        draw_text_left(
                            &mut ctx.engine,
                            &1,
                            "Waiting for opponent",
                            TextColor::White,
                            5.0,
                            25.0,
                        );
                    }
                    ConnectState::WrongVersion(..) => draw_text_left(
                        &mut ctx.engine,
                        &1,
                        "Server version is incompatible!",
                        TextColor::White,
                        5.0,
                        25.0,
                    ),
                    ConnectState::ConnectedPlay => {
                        graphics::set_canvas(ctx, self.scaler.canvas());
                        graphics::clear(ctx, Color::BLACK);
                        self.gui.draw(&mut ctx.engine, &ctx.pokedex);
                        graphics::reset_transform_matrix(ctx);
                        graphics::reset_canvas(ctx);
                        self.scaler.draw(ctx);
                    }
                    ConnectState::Closed => draw_text_left(
                        &mut ctx.engine,
                        &1,
                        "Connection Closed",
                        TextColor::White,
                        5.0,
                        5.0,
                    ),
                },
            }
        }
        Ok(())
    }
}
