# Online Pokemon Battle Engine

![image](https://user-images.githubusercontent.com/14354819/122807735-ca808800-d280-11eb-8b58-8b4d0da4b0ee.png)

## Installation

Executables may be provided under [releases](https://github.com/DoNotDoughnut/pokemon-battle-engine/releases), but they may not be up to date.
It is recommended to build the game.

## Building

1. Install Rust from [here](https://www.rust-lang.org/learn/get-started).
2. Clone the repository
3. Run ```cargo build --all``` in the repository folder (this may take a few minutes)
4. Executables will be in the target\debug directory

## Usage: 

The program uses TCP and defaults to port 28528

1. Open the server
2. Open two clients (the screen will be black on startup and say input IP address, this is normal)
3. Type the server's ip address into both clients.
4. If the screen says "Connected!" and "Waiting for opponent" you have connected. Otherwise, if the client hangs on "Connecting..." the client cannot reach the server.
5. When both clients connect, the battle starts.

## Other:

See main code for the game here: https://github.com/DoNotDoughnut/pokemon-game,
This repository is built on top of it.

Many moves are not implemented because I'm pretty sure I need to do them all manually
Also many move animations are also not in the game yet.
