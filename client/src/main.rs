#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

extern crate firecore_battle_gui as gui;
extern crate firecore_battle_net as common;
pub use gui::pokedex::engine;
use serde::{de::DeserializeOwned, Serialize};

use std::{
    fmt::Debug,
    hash::Hash,
    ops::{Deref, DerefMut},
    rc::Rc,
};

use common::{
    battle::endpoint::MpscEndpoint,
    deserialize,
    net::network::{RemoteAddr, ToRemoteAddr},
    pokedex::{item::Item, moves::Move, pokemon::Pokemon, BasicDex},
    rand::prelude::ThreadRng,
    Id, AS, DEFAULT_PORT,
};

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

use gui::pokedex::{context::PokedexClientContext, engine::{EngineContext, tetra::Event, text::TextColor}, gui::{bag::BagGui, party::PartyGui}};

use gui::BattlePlayerGui;

use log::{info, warn, LevelFilter};

use self::sender::BattleConnection;

mod sender;

const SCALE: f32 = 3.0;
const TITLE: &str = "Pokemon Battle";

fn main() -> Result {
    let l = simple_logger::SimpleLogger::new();

    #[cfg(debug_assertions)]
    let l = l.with_level(LevelFilter::Trace);

    #[cfg(not(debug_assertions))]
    let l = l.with_level(LevelFilter::Info);

    l.init()
        .unwrap_or_else(|err| panic!("Could not initialize logger with error {}", err));

    let mut engine = engine::build(
        ContextBuilder::new(TITLE, (WIDTH * SCALE) as _, (HEIGHT * SCALE) as _)
            .vsync(true)
            .resizable(true)
            .show_mouse(true),
        deserialize(include_bytes!("../fonts.bin"))
            .unwrap_or_else(|err| panic!("Could not read fonts with error {}", err)),
    )?;

    let (pokedex, movedex, itemdex) =
        deserialize::<(BasicDex<Pokemon>, BasicDex<Move>, BasicDex<Item>)>(include_bytes!(
            "../../dex.bin"
        ))
        .unwrap_or_else(|err| panic!("Could not read pokedex with error {}", err));

    let serengine: gui::pokedex::serialize::SerializedPokedexEngine =
        deserialize(include_bytes!("../dex-engine.bin"))
            .unwrap_or_else(|err| panic!("Could not read pokedex engine data with error {}", err));

    let dex = PokedexClientContext::new(&mut engine, &pokedex, &movedex, &itemdex, serengine)?;

    let mut ctx = GameContext {
        engine,
        dex,
        random: common::rand::thread_rng(),
    };

    engine::run(&mut ctx, GameState::<Id, AS>::new)
}

pub struct GameContext<'d> {
    pub engine: EngineContext,
    pub dex: PokedexClientContext<'d>,
    pub random: ThreadRng,
}

impl<'d> Deref for GameContext<'d> {
    type Target = Context;

    fn deref(&self) -> &Self::Target {
        &self.engine.tetra
    }
}

impl<'d> DerefMut for GameContext<'d> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.engine.tetra
    }
}

pub enum States<
    'd,
    ID: Default + Clone + Debug + Eq + Hash + Serialize + DeserializeOwned + Send + 'static,
    const AS: usize,
> {
    Connect(String),
    Connected(BattleConnection<'d, ID, AS>, ConnectState),
}

impl<
        'd,
        ID: Default + Clone + Debug + Eq + Hash + Serialize + DeserializeOwned + Send + 'static,
        const AS: usize,
    > States<'d, ID, AS>
{
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

struct GameState<
    'd,
    ID: Default + Clone + Debug + Eq + Hash + Serialize + DeserializeOwned + Send + 'static,
    const AS: usize,
> {
    state: States<'d, ID, AS>,
    gui: BattlePlayerGui<'d, ID, AS>,
    gui_endpoint: MpscEndpoint<ID, AS>,
    scaler: ScreenScaler,
}

impl<
        'd,
        ID: Default + Clone + Debug + Eq + Hash + Serialize + DeserializeOwned + Send + 'static,
        const AS: usize,
    > GameState<'d, ID, AS>
{
    pub fn new(ctx: &mut GameContext<'d>) -> Result<Self> {
        let party = Rc::new(PartyGui::new(&ctx.dex));
        let bag = Rc::new(BagGui::new(&ctx.dex));

        let mut gui = BattlePlayerGui::new(&mut ctx.engine.tetra, &ctx.dex, party, bag);

        let t = "rival".parse().ok();

        for remote in gui.remotes.values_mut() {
            remote.trainer = t;
        }

        let scaler = ScreenScaler::with_window_size(
            ctx,
            WIDTH as _,
            HEIGHT as _,
            ScalingMode::ShowAllPixelPerfect,
        )?;

        let gui_endpoint = gui.endpoint();

        Ok(Self {
            state: States::Connect(String::new()),
            gui,
            gui_endpoint,
            scaler,
        })
    }
}

impl<
        'd,
        ID: Default + Clone + Debug + Eq + Hash + Serialize + DeserializeOwned + Send + 'static,
        const AS: usize,
    > State<GameContext<'d>> for GameState<'d, ID, AS>
{
    fn end(&mut self, _ctx: &mut GameContext<'d>) -> Result {
        match &mut self.state {
            States::Connect(..) => (),
            States::Connected(connection, ..) => connection.end(),
        }
        Ok(())
    }

    fn update(&mut self, ctx: &mut GameContext<'d>) -> Result {
        match &mut self.state {
            States::Connect(string) => {
                if input::is_key_pressed(ctx, Key::Backspace) {
                    string.pop();
                }
                if input::is_key_pressed(ctx, Key::Enter) {
                    let mut strings = string.split_ascii_whitespace();
                    if let Some(ip) = strings.next() {

                        let mut parts = ip.split(':');
                        let ip = parts.next().unwrap();
                        let port = parts
                            .next()
                            .map(|port| port.parse::<u16>().ok())
                            .flatten()
                            .unwrap_or(DEFAULT_PORT);

                        let addr = match (ip, port).to_remote_addr() {
                            Ok(address) => match address {
                                RemoteAddr::Socket(address) => Ok(address),
                                RemoteAddr::Str(..) => Err(std::io::Error::new(
                                    std::io::ErrorKind::InvalidInput,
                                    "The address was not able to be parsed.",
                                )),
                            },
                            Err(err) => Err(err),
                        };

                        match addr {
                            Ok(addr) => {
                                info!("Connecting to server at {}", addr);
                                self.state = States::Connected(
                                    BattleConnection::connect(
                                        ctx.dex.itemdex,
                                        addr,
                                        strings.next().map(ToOwned::to_owned),
                                    ),
                                    ConnectState::WaitConfirm,
                                );
                            }
                            Err(err) => {
                                warn!("Could not parse address with error {}", err);
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
                    if let Some(connected) = connection.wait_confirm(
                        &mut ctx.random,
                        &ctx.dex,
                        time::get_delta_time(&ctx.engine.tetra).as_secs_f32(),
                    ) {
                        *state = connected;
                    }
                }
                ConnectState::Closed => self.state = States::Connect(String::new()),
                ConnectState::ConnectedWait => {
                    connection.gui_receive(&mut self.gui, &mut self.gui_endpoint, ctx, state)
                }
                ConnectState::WrongVersion(remaining) => {
                    *remaining -= time::get_delta_time(ctx).as_secs_f32();
                    if remaining < &mut 0.0 {
                        self.state = States::Connect(String::new());
                    }
                }
                ConnectState::ConnectedPlay => {
                    connection.gui_receive(&mut self.gui, &mut self.gui_endpoint, ctx, state);
                    self.gui.update(
                        &ctx.engine,
                        &ctx.dex,
                        time::get_delta_time(ctx).as_secs_f32(),
                        &mut connection.bag,
                    );
                    connection.gui_send(&mut self.gui_endpoint);
                }
            },
        }
        Ok(())
    }

    fn draw(&mut self, ctx: &mut GameContext<'d>) -> Result {
        graphics::set_canvas(ctx, self.scaler.canvas());
        graphics::clear(ctx, Color::BLACK);
        match &self.state {
            States::Connect(ip) => {
                draw_text_left(
                    &mut ctx.engine,
                    &1,
                    "Input Server Address",
                    TextColor::White,
                    5.0,
                    5.0,
                );
                draw_text_left(&mut ctx.engine, &1, ip, TextColor::White, 5.0, 25.0);
                draw_text_left(
                    &mut ctx.engine,
                    &1,
                    "Controls: X, Z, Arrow Keys",
                    TextColor::White,
                    5.0,
                    45.0,
                );
            }
            States::Connected(connection, connected) => match connected {
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
                    self.gui.draw(
                        &mut ctx.engine,
                        &ctx.dex,
                        &connection.party,
                        &connection.bag,
                    );
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
        // graphics::reset_transform_matrix(ctx);
        graphics::reset_canvas(ctx);
        graphics::clear(ctx, Color::BLACK);
        self.scaler.draw(ctx);
        Ok(())
    }

    fn event(&mut self, _: &mut GameContext<'d>, event: Event) -> Result {
        if let Event::Resized { width, height } = event {
            self.scaler.set_outer_size(width, height);
        }
        Ok(())
    }
}
