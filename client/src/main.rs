#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

extern crate firecore_battle_gui as gui;
extern crate firecore_battle_net as common;
pub use gui::pokedex::engine;
use rand::prelude::SmallRng;
use serde::{de::DeserializeOwned, Serialize};

use std::{
    fmt::Debug,
    hash::Hash,
    net::SocketAddr,
    ops::{Deref, DerefMut},
    rc::Rc,
};

use common::{
    battle::endpoint::MpscEndpoint,
    deserialize,
    pokedex::{item::Item, moves::Move, pokemon::Pokemon, BasicDex},
    Id, DEFAULT_PORT,
};

use engine::{
    graphics::{
        self, draw_text_left,
        scaling::{ScalingMode, ScreenScaler},
        Color, DrawParams,
    },
    input::{self, keyboard::Key},
    log::{info, warn},
    text::FontSheet,
    text::TextColor,
    util::{HEIGHT, WIDTH},
    Context, ContextBuilder, State,
};

use gui::{
    context::BattleGuiContext,
    pokedex::{
        context::PokedexClientData,
        gui::{bag::BagGui, party::PartyGui},
        item::bag::OwnedBag,
        pokemon::{owned::OwnedPokemon, party::Party},
        Initializable,
    },
};

use gui::BattlePlayerGui;

use self::sender::BattleConnection;

mod sender;
mod net;

const SCALE: f32 = 3.0;
const TITLE: &str = "Pokemon Battle";

static mut POKEDEX: Option<BasicDex<Pokemon>> = None;

static mut MOVEDEX: Option<BasicDex<Move>> = None;

static mut ITEMDEX: Option<BasicDex<Item>> = None;

fn main() {
    // let l = simple_logger::SimpleLogger::new();

    // #[cfg(debug_assertions)]
    // let l = l.with_level(LevelFilter::Trace);

    // #[cfg(not(debug_assertions))]
    // let l = l.with_level(LevelFilter::Info);

    // l.init()
    //     .unwrap_or_else(|err| panic!("Could not initialize logger with error {}", err));

    let fonts: Vec<FontSheet<Vec<u8>>> = deserialize(include_bytes!("../fonts.bin"))
        .unwrap_or_else(|err| panic!("Could not read fonts with error {}", err));

    let (pokedex, movedex, itemdex) =
        deserialize::<(BasicDex<Pokemon>, BasicDex<Move>, BasicDex<Item>)>(include_bytes!(
            "../../dex.bin"
        ))
        .unwrap_or_else(|err| panic!("Could not read pokedex with error {}", err));

    unsafe {
        POKEDEX = Some(pokedex);
        MOVEDEX = Some(movedex);
        ITEMDEX = Some(itemdex);
    }

    let serengine = deserialize(include_bytes!("../dex-engine.bin"))
        .unwrap_or_else(|err| panic!("Could not read pokedex engine data with error {}", err));

    engine::run(
        ContextBuilder::new(TITLE, (WIDTH * SCALE) as _, (HEIGHT * SCALE) as _), // .vsync(true).resizable(true).show_mouse(true)
        move |mut ctx| async {
            for font in fonts {
                engine::text::insert_font(&mut ctx, &font).unwrap();
            }

            let dex = PokedexClientData::new(&mut ctx, serengine)
                .await
                .unwrap_or_else(|err| {
                    panic!(
                        "Could not initialize pokedex client engine with error {}",
                        err
                    )
                });

            let btl = BattleGuiContext::new(&mut ctx).unwrap_or_else(|err| {
                panic!("Cannot initialize battle gui context with error {}", err)
            });

            let seed = (engine::inner::miniquad::date::now() * 10000.0) as u64;

            use rand::SeedableRng;

            let random = SmallRng::seed_from_u64(seed);

            GameContext {
                engine: ctx,
                btl,
                dex,
                random,
            }
        },
        GameState::<Id>::new,
    );
}

struct GameContext {
    pub engine: Context,
    pub btl: BattleGuiContext,
    pub dex: PokedexClientData,
    pub random: SmallRng,
}

impl Deref for GameContext {
    type Target = Context;

    fn deref(&self) -> &Self::Target {
        &self.engine
    }
}

impl DerefMut for GameContext {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.engine
    }
}

enum States {
    Connect(String),
    Connected(BattleConnection, ConnectState),
}

impl States {
    pub const CONNECT: Self = Self::Connect(String::new());
}

enum ConnectState {
    // WaitConnect,
    WaitConfirm,
    // WaitBegin,
    Closed,
    WrongVersion(f32),
    ConnectedWait,
    ConnectedPlay,
}

impl Default for ConnectState {
    fn default() -> Self {
        Self::WaitConfirm
    }
}

struct GameState<
    'd,
    ID: Default + Clone + Debug + Eq + Hash + Serialize + DeserializeOwned + Send + 'static,
> {
    state: States,
    player: GuiPlayer<'d>,
    gui: BattlePlayerGui<ID, &'d Pokemon, &'d Move, &'d Item>,
    gui_endpoint: MpscEndpoint<ID>,
}

struct GuiPlayer<'d> {
    pub party: Party<OwnedPokemon<&'d Pokemon, &'d Move, &'d Item>>,
    pub bag: OwnedBag<&'d Item>,
}

impl<
        'd,
        ID: Default + Clone + Debug + Eq + Hash + Serialize + DeserializeOwned + Send + 'static,
    > GameState<'d, ID>
{
    pub fn new(ctx: &mut GameContext) -> Self {
        let party = Rc::new(PartyGui::new(&ctx.dex));
        let bag = Rc::new(BagGui::new(&ctx.dex));

        let mut gui = BattlePlayerGui::new(&mut ctx.engine, &ctx.btl, party, bag);

        let t = "rival".parse().ok();

        for remote in gui.remotes.values_mut() {
            remote.npc_group = t;
        }

        let scaler = ScreenScaler::with_size(ctx, WIDTH as _, HEIGHT as _, ScalingMode::Stretch);

        graphics::scaling::set_scaler(ctx, scaler);

        let gui_endpoint = gui.endpoint().clone();

        Self {
            state: States::CONNECT,
            gui,
            player: GuiPlayer {
                party: Default::default(),
                bag: vec![gui::pokedex::item::SavedItemStack::new(
                    "hyper_potion".parse().unwrap(),
                    2,
                )]
                .init(unsafe { ITEMDEX.as_ref().unwrap() })
                .unwrap(),
            },
            gui_endpoint: gui_endpoint,
        }
    }
}

impl<
        'd,
        ID: Default + Clone + Debug + Eq + Hash + Serialize + DeserializeOwned + Send + 'static,
    > State<GameContext> for GameState<'d, ID>
{
    fn end(&mut self, _ctx: &mut GameContext) {
        match &mut self.state {
            States::Connect(..) => (),
            States::Connected(connection, ..) => {
                self.gui.forfeit();
                connection.end::<ID>();
            }
        }
    }

    fn update(&mut self, ctx: &mut GameContext, delta: f32) {
        match &mut self.state {
            States::Connect(string) => {
                if input::keyboard::is_key_pressed(ctx, Key::Backspace) {
                    string.pop();
                }
                if input::keyboard::is_key_pressed(ctx, Key::Enter) {
                    let mut strings = string.split_ascii_whitespace();
                    match strings.next() {
                        Some(addr) => match find_address(parse_address(addr)) {
                            Ok(addr) => {
                                info!("Connecting to server at {}", addr);
                                match BattleConnection::connect(
                                    addr,
                                    strings.next().map(ToOwned::to_owned),
                                    // strings.next().map(|s| s.parse().ok()).flatten(),
                                ) {
                                    Some(connection) => {
                                        self.state =
                                            States::Connected(connection, ConnectState::default())
                                    }
                                    None => {
                                        warn!("Could not create connection!");
                                        string.clear();
                                    }
                                }
                            }
                            Err(err) => {
                                warn!("Could not parse address with error {}", err);
                                string.clear();
                            }
                        },
                        None => warn!("No text was input for server address."),
                    }
                // } else if let Some(new) = input::get_text_input(ctx) {
                //     string.push_str(new);
                // }
                } else {
                    while let Some(c) = engine::inner::prelude::get_char_pressed() {
                        string.push(c);
                    }
                }
            }
            States::Connected(connection, state) => match state {
                // ConnectState::WaitConnect => {
                //     if connection.connected() {
                //         *state = ConnectState::WaitConfirm;
                //     }
                // }
                ConnectState::WaitConfirm => {
                    if let Some(connected) =
                        connection.wait_confirm::<ID>(ctx, &mut self.player, delta)
                    {
                        *state = connected;
                    }
                }
                ConnectState::Closed => self.state = States::Connect(String::new()),
                ConnectState::ConnectedWait => connection.gui_receive(
                    &mut self.gui,
                    &mut self.player,
                    &mut self.gui_endpoint,
                    ctx,
                    state,
                ),
                ConnectState::WrongVersion(remaining) => {
                    *remaining -= delta;
                    if remaining < &mut 0.0 {
                        self.state = States::Connect(String::new());
                    }
                }
                ConnectState::ConnectedPlay => {
                    connection.gui_receive(
                        &mut self.gui,
                        &mut self.player,
                        &mut self.gui_endpoint,
                        ctx,
                        state,
                    );
                    let pokedex = unsafe { crate::POKEDEX.as_ref().unwrap() };
                    let movedex = unsafe { crate::MOVEDEX.as_ref().unwrap() };
                    let itemdex = unsafe { crate::ITEMDEX.as_ref().unwrap() };
                    self.gui.update(
                        &ctx.engine,
                        &ctx.dex,
                        pokedex,
                        movedex,
                        itemdex,
                        delta,
                        &mut self.player.bag,
                    );
                    connection.gui_send(&mut self.gui_endpoint);
                }
            },
        }
    }

    fn draw(&mut self, ctx: &mut GameContext) {
        // graphics::set_canvas(ctx, self.scaler.canvas());
        graphics::clear(ctx, Color::BLACK);
        match &self.state {
            States::Connect(ip) => {
                let params = DrawParams::color(TextColor::White.into());
                draw_text_left(
                    &mut ctx.engine,
                    &1,
                    "Input Server Address",
                    5.0,
                    5.0,
                    params,
                );
                draw_text_left(&mut ctx.engine, &1, ip, 5.0, 25.0, params);
                draw_text_left(
                    &mut ctx.engine,
                    &1,
                    "Controls: X is (A), Z is (B)",
                    5.0,
                    45.0,
                    params,
                );
                draw_text_left(
                    &mut ctx.engine,
                    &1,
                    "Arrow Keys are D-Pad",
                    5.0,
                    65.0,
                    params,
                );
            }
            States::Connected(.., connected) => match connected {
                ConnectState::WaitConfirm => draw_text_left(
                    &mut ctx.engine,
                    &1,
                    "Connecting...",
                    5.0,
                    5.0,
                    DrawParams::color(TextColor::White.into()),
                ),
                ConnectState::ConnectedWait => {
                    draw_text_left(
                        &mut ctx.engine,
                        &1,
                        "Connected!",
                        5.0,
                        5.0,
                        DrawParams::color(TextColor::White.into()),
                    );
                    draw_text_left(
                        &mut ctx.engine,
                        &1,
                        "Waiting for opponent",
                        5.0,
                        25.0,
                        DrawParams::color(TextColor::White.into()),
                    );
                }
                ConnectState::WrongVersion(..) => draw_text_left(
                    &mut ctx.engine,
                    &1,
                    "Server version is incompatible!",
                    5.0,
                    25.0,
                    DrawParams::color(TextColor::White.into()),
                ),
                ConnectState::ConnectedPlay => {
                    self.gui.draw(
                        &mut ctx.engine,
                        &ctx.dex,
                        &self.player.party,
                        &self.player.bag,
                    );
                }
                ConnectState::Closed => draw_text_left(
                    &mut ctx.engine,
                    &1,
                    "Connection Closed",
                    5.0,
                    5.0,
                    DrawParams::color(TextColor::White.into()),
                ),
            },
        }
        // graphics::reset_canvas(ctx);
        // graphics::clear(ctx, Color::BLACK);
        // self.scaler.draw(ctx);
        // Ok(())
    }

    // fn event(&mut self, _: &mut GameContext<'d>, event: Event) -> Result {
    //     if let Event::Resized { width, height } = event {
    //         self.scaler.set_outer_size(width, height);
    //     }
    //     Ok(())
    // }
}

fn parse_address(addr: &str) -> (&str, u16) {
    let mut parts = addr.split(':');
    let addr = parts.next().unwrap();
    let port = parts
        .next()
        .map(|port| port.parse::<u16>().ok())
        .flatten()
        .unwrap_or(DEFAULT_PORT);
    (addr, port)
}

fn find_address(addr: (&str, u16)) -> Result<SocketAddr, std::io::Error> {
    // use message_io::network::{RemoteAddr, ToRemoteAddr};
    // match addr.to_remote_addr() {
    //     Ok(address) => match address {
    //         RemoteAddr::Socket(address) => Ok(address),
    //         RemoteAddr::Str(..) => Err(std::io::Error::new(
    //             std::io::ErrorKind::InvalidInput,
    //             "The address was not able to be parsed.",
    //         )),
    //     },
    //     Err(err) => Err(err),
    // }
    // #[cfg(not(target_arch = "wasm32"))]
    // let ws = "";

    use std::net::ToSocketAddrs;

    Ok(format!("{}:{}", addr.0, addr.1).to_socket_addrs().unwrap().next().unwrap())
    // Ok(SocketAddr::new(
    //     addr.0
    //         .parse()
    //         .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidInput, err))?,
    //     addr.1,
    // ))
}
