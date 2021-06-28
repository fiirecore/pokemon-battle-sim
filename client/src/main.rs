#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

extern crate firecore_battle_net as common;
extern crate firecore_game as game;
extern crate firecore_battle_client as client;

use std::{
    net::{IpAddr, SocketAddr},
    rc::Rc,
};

use common::{borrow::BorrowableMut, pokedex::item::{ItemStack, bag::Bag}, uuid::Uuid};

use game::{
    deps::ser,
    graphics::draw_text_left,
    gui::{bag::BagGui, party::PartyGui},
    init,
    log::{info, warn},
    tetra::{
        graphics::{
            self,
            scaling::{ScalingMode, ScreenScaler},
            Color,
        },
        input::{self, Key},
        time::{self, Timestep},
        Context, ContextBuilder, Event, Result, State,
    },
    util::{HEIGHT, WIDTH},
};

use client::BattlePlayerGui;

use self::sender::BattleConnection;

mod sender;

const SCALE: f32 = 3.0;
const TITLE: &str = "Pokemon Battle";

fn main() -> Result {
    game::init::logger();

    ContextBuilder::new(TITLE, (WIDTH * SCALE) as _, (HEIGHT * SCALE) as _)
        .vsync(true)
        .resizable(true)
        .show_mouse(true)
        .timestep(Timestep::Variable)
        .build()?
        .run(GameState::new)
}

pub enum States {
    Connect(String),
    Connected(BattleConnection, ConnectState),
}

pub enum ConnectState {
    WaitConfirm,
    // WaitBegin,
    Closed,
    ConnectedWait,
    ConnectedPlay,
}

struct GameState {
    state: States,
    gui: BattlePlayerGui<Uuid>,
    scaler: ScreenScaler,
}

impl GameState {
    pub fn new(ctx: &mut Context) -> Result<Self> {
        let party = Rc::new(PartyGui::new(ctx));
        let bag = Rc::new(BagGui::new(ctx));

        let scaler =
            ScreenScaler::with_window_size(ctx, WIDTH as _, HEIGHT as _, ScalingMode::ShowAll)?;
        Ok(Self {
            state: States::Connect(String::new()),
            gui: BattlePlayerGui::new(
                ctx,
                party,
                bag,
                BorrowableMut::Owned(Bag { items: vec![ItemStack::new(&"hyper_potion".parse().unwrap(), 3)] }),
                Uuid::default(),
            ),
            scaler,
        })
    }
}

impl State for GameState {
    fn begin(&mut self, ctx: &mut Context) -> Result {
        init::configuration()?;
        init::text(
            ctx,
            ser::deserialize(include_bytes!("../fonts.bin")).unwrap(),
        )?;
        init::pokedex(ctx, ser::deserialize(include_bytes!("../dex.bin")).unwrap())
    }

    fn end(&mut self, _ctx: &mut Context) -> Result {
        match &mut self.state {
            States::Connect(..) => (),
            States::Connected(connection, ..) => connection.end(),
        }
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
                    if let Some(connected) = connection.wait_confirm() {
                        *state = connected;
                    }
                }
                ConnectState::Closed => self.state = States::Connect(String::new()),
                ConnectState::ConnectedWait => connection.gui_receive(&mut self.gui, ctx, state),
                ConnectState::ConnectedPlay => {
                    connection.gui_receive(&mut self.gui, ctx, state);
                    self.gui
                        .update(ctx, time::get_delta_time(ctx).as_secs_f32(), false);
                    connection.gui_send(&mut self.gui);
                }
            },
        }
        Ok(())
    }

    fn draw(&mut self, ctx: &mut Context) -> Result {
        graphics::clear(ctx, Color::BLACK);
        {
            match &self.state {
                States::Connect(ip) => {
                    draw_text_left(ctx, &1, "Input IP Address", &Color::WHITE, 5.0, 5.0);
                    draw_text_left(ctx, &1, ip, &Color::WHITE, 5.0, 25.0);
                }
                States::Connected(.., connected) => match connected {
                    ConnectState::WaitConfirm => {
                        draw_text_left(ctx, &1, "Connecting...", &Color::WHITE, 5.0, 5.0)
                    }
                    ConnectState::ConnectedWait => {
                        draw_text_left(ctx, &1, "Connected!", &Color::WHITE, 5.0, 5.0);
                        draw_text_left(ctx, &1, "Waiting for opponent", &Color::WHITE, 5.0, 25.0);
                    }
                    ConnectState::ConnectedPlay => {
                        graphics::set_canvas(ctx, self.scaler.canvas());
                        graphics::clear(ctx, Color::BLACK);
                        self.gui.draw(ctx);
                        graphics::reset_transform_matrix(ctx);
                        graphics::reset_canvas(ctx);
                        self.scaler.draw(ctx);
                    }
                    ConnectState::Closed => {
                        draw_text_left(ctx, &1, "Connection Closed", &Color::WHITE, 5.0, 5.0)
                    }
                },
            }
        }
        Ok(())
    }

    fn event(&mut self, _ctx: &mut Context, _event: Event) -> Result {
        Ok(())
    }
}
